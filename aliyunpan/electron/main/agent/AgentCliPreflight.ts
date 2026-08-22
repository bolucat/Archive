import type { AgentPlanAction, AgentResourceHandle, AgentResourceSnapshot, AgentWorkflowView } from '@shared/types/agentControl'
import { cloudDriveCliAccountId, cloudDriveCliProviderId, type CloudDriveCliAdapter } from './CloudDriveCliAdapter'

export interface AgentCliActionPreflight {
  command: 'files move-apply' | 'files trash-apply'
  rationale: string
  plan: Record<string, unknown>
}

type ResourceInspector = Pick<CloudDriveCliAdapter, 'inspectResource' | 'isWithinScopeRoot'>

/**
 * Re-read every resource immediately before execution and turn exactly one approved
 * action into a CLI plan. This function never performs a cloud write.
 */
export async function preflightAgentCliAction(workflow: AgentWorkflowView, action: AgentPlanAction, inspector: ResourceInspector, now = Date.now()): Promise<AgentCliActionPreflight> {
  if (!workflow.plan || workflow.plan.status !== 'approved') throw new Error('只能预检已批准的 Agent 计划')
  const handle = resolveSingleHandle(workflow.resourceHandles, action)
  const current = await inspector.inspectResource(workflow.scope, handle.snapshot.resourceId)
  assertSnapshotStillMatches(handle.snapshot, current.snapshot)
  if (!await inspector.isWithinScopeRoot(workflow.scope, handle.snapshot.resourceId)) throw new Error('资源已不在授权根目录内，需要重新取证')

  const provider = cloudDriveCliProviderId(workflow.scope.platform)
  const accountId = cloudDriveCliAccountId(workflow.scope)
  const createdAt = new Date(now).toISOString()
  if (action.kind === 'trash') {
    return {
      command: 'files trash-apply',
      rationale: `Approved Agent plan ${workflow.plan.id}: ${action.label}`,
      plan: { version: 1, operation: 'trash', provider, account_id: accountId, created_at: createdAt, items: [{ drive_id: workflow.scope.driveId, file_id: handle.snapshot.resourceId, name: handle.snapshot.name, type: current.kind, parent_file_id: handle.snapshot.parentResourceId || '', reason: action.label }] }
    }
  }
  if (action.kind === 'move') {
    const targetParentFileId = String(action.parameters?.targetParentFileId || '')
    if (!targetParentFileId) throw new Error('移动计划缺少目标目录')
    const target = await inspector.inspectResource(workflow.scope, targetParentFileId)
    if (target.kind !== 'folder') throw new Error('移动目标不是目录')
    if (!await inspector.isWithinScopeRoot(workflow.scope, targetParentFileId)) throw new Error('移动目标不在授权根目录内')
    if (!handle.snapshot.parentResourceId) throw new Error('移动计划缺少原始父目录')
    return {
      command: 'files move-apply',
      rationale: `Approved Agent plan ${workflow.plan.id}: ${action.label}`,
      plan: { version: 1, operation: 'move', provider, account_id: accountId, created_at: createdAt, items: [{ drive_id: workflow.scope.driveId, file_id: handle.snapshot.resourceId, name: handle.snapshot.name, type: current.kind, from_parent_file_id: handle.snapshot.parentResourceId, to_parent_file_id: targetParentFileId, reason: action.label }] }
    }
  }
  throw new Error(`Agent V1 执行器尚不支持 ${action.kind} 操作`)
}

function resolveSingleHandle(handles: AgentResourceHandle[], action: AgentPlanAction): AgentResourceHandle {
  if (action.resourceHandleIds.length !== 1) throw new Error('当前 Agent V1 操作必须精确绑定一个资源句柄')
  const handle = handles.find(item => item.id === action.resourceHandleIds[0])
  if (!handle) throw new Error('计划引用的资源句柄不存在')
  return handle
}

function assertSnapshotStillMatches(expected: AgentResourceSnapshot, actual: AgentResourceSnapshot): void {
  if (expected.accountId !== actual.accountId || expected.driveId !== actual.driveId || expected.resourceId !== actual.resourceId) throw new Error('资源身份已变化，需要重新取证')
  if (expected.name !== actual.name) throw new Error('资源名称已变化，需要重新取证')
  if (expected.parentResourceId && expected.parentResourceId !== actual.parentResourceId) throw new Error('资源所在目录已变化，需要重新取证')
}
