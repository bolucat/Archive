import type { AgentPlanAction, AgentResourceHandle, AgentScopeGrant, AgentWorkflowView, CreateAgentPlanInput } from '@shared/types/agentControl'
import type { WorkspacePlanAction, WorkspaceTaskView } from '@shared/types/workspaceAgent'
import { addAgentEvidence, approveAgentPlan, completeExternalAgentWorkflow, createAgentPlan, createAgentResourceHandle, createAgentWorkflow, findAgentWorkflowByExternalRef, replaceAgentDraftPlan } from '../agent/AgentControlService'
import { agentV1FeatureFlags } from '../agent/AgentV1FeatureFlags'

const workflowRef = (taskId: string) => `workspace:${taskId}`
const evidenceRef = (evidenceId: string) => `workspace-evidence:${evidenceId}`
const actionRef = (actionId: string) => `workspace-action:${actionId}`

export interface WorkspaceAgentV1Audit {
  cutover: boolean
  workflowId: string
  workflowStatus: AgentWorkflowView['status']
  plan?: { hash: string; status: string }
  grant?: { status: string; expiresAt: number }
}

export function getWorkspaceAgentV1Audit(taskId: string): WorkspaceAgentV1Audit | null {
  if (!agentV1FeatureFlags().workspaceBridge) return null
  const workflow = findAgentWorkflowByExternalRef(workflowRef(taskId))
  if (!workflow) return null
  return {
    cutover: agentV1FeatureFlags().workspaceCutover,
    workflowId: workflow.id,
    workflowStatus: workflow.status,
    plan: workflow.plan ? { hash: workflow.plan.hash, status: workflow.plan.status } : undefined,
    grant: workflow.executionGrant ? { status: workflow.executionGrant.status, expiresAt: workflow.executionGrant.expiresAt } : undefined
  }
}

export function workspaceAgentScope(task: Pick<WorkspaceTaskView, 'kind' | 'scope'>): AgentScopeGrant {
  const operations = ['files.list']
  if (task.kind === 'organize_files') operations.push('files.move')
  if (task.kind === 'cleanup_duplicates' || task.kind === 'cleanup_large_files' || task.kind === 'cleanup_empty_directories') operations.push('trash.move')
  if (task.kind === 'download_files') operations.push('files.download')
  if (task.kind === 'import_share') operations.push('share.import')
  return { accountId: task.scope.userId, driveId: task.scope.driveId, rootId: task.scope.rootId, platform: task.scope.platform, operations }
}

export function workspacePlanActionToAgentAction(task: WorkspaceTaskView, action: WorkspacePlanAction, handle: AgentResourceHandle): AgentPlanAction {
  const parameters: Record<string, unknown> = {}
  if (action.kind === 'move') parameters.targetParentFileId = action.targetParentFileId
  if (action.kind === 'import_share') {
    parameters.shareId = action.share?.shareId
    parameters.fileIds = action.share?.fileIds
  }
  return {
    id: `workspace:${action.id}`,
    kind: action.kind,
    label: action.label,
    idempotencyKey: `workspace:${task.id}:${action.id}`,
    resourceHandleIds: [handle.id],
    parameters: Object.keys(parameters).length ? parameters : undefined
  }
}

export function isWorkspaceAgentV1ExecutableAction(action: Pick<WorkspacePlanAction, 'kind'>): boolean {
  return action.kind === 'move' || action.kind === 'trash'
}

export function mirrorWorkspaceTask(task: WorkspaceTaskView): void {
  if (!agentV1FeatureFlags().workspaceBridge || findAgentWorkflowByExternalRef(workflowRef(task.id))) return
  createAgentWorkflow({ surface: 'workspace', goal: task.goal, scope: workspaceAgentScope(task), externalRef: workflowRef(task.id) })
}

export function mirrorWorkspaceEvidence(task: WorkspaceTaskView): void {
  if (!agentV1FeatureFlags().workspaceBridge) return
  const workflow = requireMirroredWorkflow(task)
  if (workflow.status !== 'gathering_evidence') return
  for (const evidence of task.evidence) {
    addAgentEvidence(workflow.id, { source: evidence.source, summary: evidence.summary, data: evidence.data, externalRef: evidenceRef(evidence.id) })
  }
}

export function mirrorWorkspacePlan(task: WorkspaceTaskView): void {
  if (!agentV1FeatureFlags().workspaceBridge || !task.plan) return
  const workflow = requireMirroredWorkflow(task)
  mirrorWorkspaceEvidence(task)
  const refreshed = findAgentWorkflowByExternalRef(workflowRef(task.id))!
  if (refreshed.status !== 'gathering_evidence' && refreshed.status !== 'awaiting_approval') return
  const planInput = toAgentPlanInput(task, refreshed)
  if (!refreshed.plan) createAgentPlan(refreshed.id, planInput)
  else if (refreshed.plan.status === 'awaiting_approval') replaceAgentDraftPlan(refreshed.id, planInput)
}

export function mirrorWorkspaceApproval(task: WorkspaceTaskView): void {
  if (!agentV1FeatureFlags().workspaceBridge || !task.plan || task.plan.status !== 'approved') return
  if (agentV1FeatureFlags().workspaceCutover && task.plan.actions.some(action => !isWorkspaceAgentV1ExecutableAction(action))) return
  const workflow = requireMirroredWorkflow(task)
  if (workflow.plan?.status !== 'awaiting_approval') return
  approveAgentPlan(workflow.id, workflow.plan.hash)
}

export function mirrorWorkspaceTerminal(task: WorkspaceTaskView): void {
  if (!agentV1FeatureFlags().workspaceBridge || !['completed', 'partial', 'failed', 'stale', 'cancelled'].includes(task.status)) return
  const workflow = findAgentWorkflowByExternalRef(workflowRef(task.id))
  if (!workflow || ['completed', 'failed', 'cancelled', 'expired'].includes(workflow.status)) return
  const status = task.status === 'completed' ? 'completed' : task.status === 'cancelled' ? 'cancelled' : task.status === 'stale' ? 'expired' : 'failed'
  completeExternalAgentWorkflow(workflow.id, status, task.summary || `工作台任务已${status === 'completed' ? '完成' : status === 'cancelled' ? '取消' : '结束'}。`)
}

function requireMirroredWorkflow(task: WorkspaceTaskView): AgentWorkflowView {
  mirrorWorkspaceTask(task)
  const workflow = findAgentWorkflowByExternalRef(workflowRef(task.id))
  if (!workflow) throw new Error('未能创建 Agent V1 工作流镜像')
  return workflow
}

function toAgentPlanInput(task: WorkspaceTaskView, workflow: AgentWorkflowView): CreateAgentPlanInput {
  const plan = task.plan!
  const evidenceByRef = new Map(workflow.evidence.map(item => [item.externalRef, item]))
  const evidenceIds = plan.evidenceIds.map(id => evidenceByRef.get(evidenceRef(id))?.id).filter((id): id is string => !!id)
  if (evidenceIds.length !== plan.evidenceIds.length) throw new Error('工作台计划缺少对应的已验证证据')
  const evidenceId = evidenceIds[0]
  const handlesByRef = new Map(workflow.resourceHandles.map(item => [item.externalRef, item]))
  const actions = plan.actions.map(action => {
    let handle = handlesByRef.get(actionRef(action.id))
    if (!handle) {
      handle = createAgentResourceHandle(workflow.id, {
        evidenceId,
        kind: action.kind === 'import_share' ? 'share' : 'file',
        snapshot: actionSnapshot(task, action),
        externalRef: actionRef(action.id)
      })
    }
    return workspacePlanActionToAgentAction(task, action, handle)
  })
  return { summary: plan.summary, risk: plan.risk, evidenceIds, actions }
}

function actionSnapshot(task: WorkspaceTaskView, action: WorkspacePlanAction) {
  if (action.kind === 'import_share') {
    if (!action.share?.shareId) throw new Error('分享导入计划缺少已验证的分享标识')
    return { accountId: task.scope.userId, driveId: task.scope.driveId, resourceId: action.share.shareId, name: action.label }
  }
  const snapshot = action.snapshot
  if (!snapshot || snapshot.userId !== task.scope.userId || snapshot.driveId !== task.scope.driveId) throw new Error('工作台计划资源超出当前网盘范围')
  return { accountId: snapshot.userId, driveId: snapshot.driveId, resourceId: snapshot.fileId, parentResourceId: snapshot.parentFileId, name: snapshot.name }
}
