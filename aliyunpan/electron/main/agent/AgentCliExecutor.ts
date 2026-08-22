import { app } from 'electron'
import { mkdtemp, rm, writeFile } from 'fs/promises'
import { join } from 'path'
import { claimAgentExecutionGrant, getAgentWorkflow, recordAgentActionReceipt } from './AgentControlService'
import { preflightAgentCliAction } from './AgentCliPreflight'
import type { AgentCliActionPreflight } from './AgentCliPreflight'
import { CloudDriveCliAdapter } from './CloudDriveCliAdapter'
import { runBundledCloudDriveCliJson } from './CloudDriveCliGateway'
import { agentV1FeatureFlags } from './AgentV1FeatureFlags'

export interface AgentCliExecutionResult {
  grantId?: string
  workflowId?: string
  executedActions: number
  status: 'idle' | 'completed' | 'failed'
}

/**
 * Main-process-only executor. It intentionally has no IPC handler: a renderer
 * may create/approve a plan but cannot claim or consume an execution grant.
 */
export async function executeNextAgentCliGrant(workerId = `main:${process.pid}`): Promise<AgentCliExecutionResult> {
  if (!agentV1FeatureFlags().cliExecutor) throw new Error('Agent V1 CLI 执行器尚未启用')
  const grant = claimAgentExecutionGrant(workerId, undefined, ['workspace', 'ai_search'])
  if (!grant) return { executedActions: 0, status: 'idle' }
  const workflow = getAgentWorkflow(grant.workflowId)
  if (!workflow?.plan || workflow.plan.id !== grant.planId || workflow.plan.hash !== grant.planHash) throw new Error('执行授权与当前计划不匹配')

  const inspector = new CloudDriveCliAdapter()
  let executedActions = 0
  for (const action of workflow.plan.actions) {
    try {
      const preflight = await preflightAgentCliAction(workflow, action, inspector)
      const result = await applyCliPreflight(preflight)
      recordAgentActionReceipt(grant.id, workerId, { actionId: action.id, idempotencyKey: action.idempotencyKey, status: 'succeeded', result })
      executedActions++
    } catch (error: any) {
      const message = error?.message || 'Agent CLI 操作失败'
      recordAgentActionReceipt(grant.id, workerId, { actionId: action.id, idempotencyKey: action.idempotencyKey, status: 'failed', result: { message } })
      return { grantId: grant.id, workflowId: workflow.id, executedActions, status: 'failed' }
    }
  }
  return { grantId: grant.id, workflowId: workflow.id, executedActions, status: 'completed' }
}

async function applyCliPreflight(preflight: AgentCliActionPreflight): Promise<Record<string, unknown>> {
  const directory = await mkdtemp(join(app.getPath('temp'), 'boxplayer-agent-v1-'))
  const planPath = join(directory, 'approved-plan.json')
  try {
    await writeFile(planPath, JSON.stringify(preflight.plan), 'utf8')
    const args = preflight.command === 'files trash-apply'
      ? ['files', 'trash-apply', planPath, '--apply', '--rationale', preflight.rationale]
      : ['files', 'move-apply', planPath, '--rationale', preflight.rationale]
    return await runBundledCloudDriveCliJson<Record<string, unknown>>(args)
  } finally {
    await rm(directory, { recursive: true, force: true })
  }
}
