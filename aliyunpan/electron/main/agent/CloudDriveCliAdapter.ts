import type { AgentResourceSnapshot, AgentScopeGrant } from '@shared/types/agentControl'
import { runBundledCloudDriveCliJson } from './CloudDriveCliGateway'

export interface CloudDriveCliFile {
  provider?: string
  accountId?: string
  driveId?: string
  fileId?: string
  parentFileId?: string
  name?: string
  type?: 'file' | 'folder'
}

export interface CloudDriveCliDirectoryStats {
  provider?: string
  driveId?: string
  path?: string
  max_depth?: number
  total_files?: number
  total_dirs?: number
  total_size?: number
  by_category?: Record<string, { count?: number; size?: number }>
  top_extensions?: Record<string, number>
}

export function cloudDriveCliScopeArgs(scope: AgentScopeGrant): string[] {
  return ['--provider', cloudDriveCliProviderId(scope.platform), '--account', cloudDriveCliAccountId(scope), '--drive-id', scope.driveId]
}

export function cloudDriveCliStatsArgs(scope: AgentScopeGrant, maxDepth: number): string[] {
  const depth = Math.max(1, Math.min(2, Math.floor(maxDepth || 1)))
  return ['files', 'stats', '--file-id', scope.rootId, '--depth', String(depth), ...cloudDriveCliScopeArgs(scope)]
}

export function toAgentResourceSnapshot(file: CloudDriveCliFile, scope: AgentScopeGrant): AgentResourceSnapshot {
  if (!file.fileId || !file.name) throw new Error('网盘 CLI 返回的文件信息不完整')
  if (file.accountId && file.accountId !== cloudDriveCliAccountId(scope)) throw new Error('网盘 CLI 返回了其他账号的资源')
  if (file.driveId && file.driveId !== scope.driveId) throw new Error('网盘 CLI 返回了其他网盘的资源')
  return { accountId: scope.accountId, driveId: scope.driveId, resourceId: file.fileId, parentResourceId: file.parentFileId || undefined, name: file.name }
}

export function cloudDriveCliAccountId(scope: AgentScopeGrant): string {
  const provider = cloudDriveCliProviderId(scope.platform)
  if (provider === 'aliyun' && !scope.accountId.startsWith('aliyun_')) return `aliyun_${scope.accountId}`
  return scope.accountId
}

export function cloudDriveCliProviderId(platform: string): string {
  const normalized = String(platform || '').toLowerCase()
  if (normalized === '123') return 'cloud123'
  if (normalized === 'drive115') return '115'
  if (normalized === 'cloud139') return '139'
  if (normalized === 'cloud189') return '189'
  return normalized || 'aliyun'
}

export class CloudDriveCliAdapter {
  async inspectResource(scope: AgentScopeGrant, resourceId: string): Promise<{ kind: 'file' | 'folder'; snapshot: AgentResourceSnapshot }> {
    if (!scope.operations.includes('files.list')) throw new Error('当前范围未授予 files.list 操作权限')
    const file = await this.loadFile(scope, resourceId)
    return { kind: file.type === 'folder' ? 'folder' : 'file', snapshot: toAgentResourceSnapshot(file, scope) }
  }

  /** Resolve the parent chain at execution time; an account/drive match alone is not enough scope proof. */
  async isWithinScopeRoot(scope: AgentScopeGrant, resourceId: string): Promise<boolean> {
    if (!scope.operations.includes('files.list')) throw new Error('当前范围未授予 files.list 操作权限')
    const visited = new Set<string>()
    let currentId = resourceId
    for (let depth = 0; depth < 64; depth++) {
      if (currentId === scope.rootId) return true
      if (!currentId || visited.has(currentId)) return false
      visited.add(currentId)
      const current = await this.loadFile(scope, currentId)
      currentId = current.parentFileId || ''
    }
    return false
  }

  async getDirectoryStats(scope: AgentScopeGrant, maxDepth = 2): Promise<Required<Pick<CloudDriveCliDirectoryStats, 'provider' | 'driveId' | 'path' | 'max_depth' | 'total_files' | 'total_dirs' | 'total_size' | 'by_category' | 'top_extensions'>>> {
    if (!scope.operations.includes('files.list')) throw new Error('当前范围未授予 files.list 操作权限')
    const stats = await runBundledCloudDriveCliJson<CloudDriveCliDirectoryStats>(cloudDriveCliStatsArgs(scope, maxDepth))
    if (stats.provider && cloudDriveCliProviderId(stats.provider) !== cloudDriveCliProviderId(scope.platform)) throw new Error('网盘 CLI 返回了其他平台的统计结果')
    if (stats.driveId && stats.driveId !== scope.driveId) throw new Error('网盘 CLI 返回了其他网盘的统计结果')
    if (stats.path && stats.path !== scope.rootId) throw new Error('网盘 CLI 返回了授权范围外的统计结果')
    return {
      provider: cloudDriveCliProviderId(scope.platform),
      driveId: scope.driveId,
      path: scope.rootId,
      max_depth: Math.max(1, Math.min(2, Math.floor(maxDepth || 1))),
      total_files: Number(stats.total_files || 0),
      total_dirs: Number(stats.total_dirs || 0),
      total_size: Number(stats.total_size || 0),
      by_category: stats.by_category || {},
      top_extensions: stats.top_extensions || {}
    }
  }

  private async loadFile(scope: AgentScopeGrant, resourceId: string): Promise<CloudDriveCliFile> {
    return await runBundledCloudDriveCliJson<CloudDriveCliFile>(['files', 'info', '--file-id', resourceId, ...cloudDriveCliScopeArgs(scope)])
  }
}
