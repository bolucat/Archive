import type { AgentPlanAction, AgentResourceHandle, AgentScopeGrant } from './types/agentControl'

const REQUIRED_OPERATION: Record<AgentPlanAction['kind'], string> = {
  move: 'files.move',
  trash: 'trash.move',
  import_share: 'share.import',
  download: 'files.download',
  offline_download: 'offline.create'
}

export function validateAgentPlanPolicy(scope: AgentScopeGrant, actions: AgentPlanAction[], handles: AgentResourceHandle[]): string | null {
  const byId = new Map(handles.map(handle => [handle.id, handle]))
  for (const action of actions) {
    const requiredOperation = REQUIRED_OPERATION[action.kind]
    if (!scope.operations.includes(requiredOperation)) return `当前范围未授予 ${requiredOperation} 操作权限`
    const actionHandles = action.resourceHandleIds.map(id => byId.get(id)).filter((handle): handle is AgentResourceHandle => !!handle)
    if (!actionHandles.length || actionHandles.length !== action.resourceHandleIds.length) return `操作“${action.label}”引用了无效资源句柄`
    if (actionHandles.some(handle => handle.snapshot.accountId !== scope.accountId || handle.snapshot.driveId !== scope.driveId)) return `操作“${action.label}”超出已授予的网盘范围`
    if (action.kind === 'import_share' && actionHandles.some(handle => handle.kind !== 'share')) return `分享导入只能引用已取证的分享资源`
    if (action.kind === 'offline_download' && actionHandles.some(handle => handle.kind !== 'external_source')) return `离线下载只能引用已取证的外部资源`
    if (!['import_share', 'offline_download'].includes(action.kind) && actionHandles.some(handle => !['file', 'folder'].includes(handle.kind))) return `操作“${action.label}”只能引用文件或文件夹资源`
  }
  return null
}
