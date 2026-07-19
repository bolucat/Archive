import { apiCloud123OfflineCreate } from '../../cloud123/offline'
import { apiDrive115OfflineCreate } from '../../cloud115/offline'
import { apiGuangyaOfflineCreate } from '../../guangya/offline'
import { apiPikPakOfflineCreate } from '../../pikpak/offline'
import type { MediaAcquisitionTarget } from '@shared/types/mediaAcquisition'
import { normalizeMediaAcquisitionPlatform, normalizeMediaAcquisitionRootFolder } from './capabilities'

export interface ExternalUrlOfflineSubmission {
  message: string
  platform: string
  taskId?: string
  fileId?: string
}

export async function submitExternalUrlOffline(target: MediaAcquisitionTarget, url: string, fileName: string): Promise<ExternalUrlOfflineSubmission> {
  const platform = normalizeMediaAcquisitionPlatform(target.targetPlatform)
  const parentId = normalizeMediaAcquisitionRootFolder(platform, target.targetParentFileId)
  if (platform === '115') {
    const result = await apiDrive115OfflineCreate(target.targetUserId, url, parentId)
    if (result.error) throw new Error(result.error)
    return { message: '已提交 115 字幕离线下载', platform, taskId: result.taskIds[0] }
  }
  if (platform === 'guangya') {
    const result = await apiGuangyaOfflineCreate(target.targetUserId, url, fileName, parentId)
    if (result.error) throw new Error(result.error)
    return { message: '已提交光鸭字幕离线下载', platform, taskId: result.taskId, fileId: result.fileId }
  }
  if (platform === 'pikpak') {
    const result = await apiPikPakOfflineCreate(target.targetUserId, url, fileName, parentId)
    if (result.error) throw new Error(result.error)
    return { message: '已提交 PikPak 字幕离线下载', platform, taskId: result.taskId || undefined, fileId: result.fileId }
  }
  if (platform === 'cloud123') {
    const result = await apiCloud123OfflineCreate(target.targetUserId, url, fileName, parentId)
    if (result.error) throw new Error(result.error)
    return { message: '已提交 123 云盘字幕离线下载', platform, taskId: result.taskId ? String(result.taskId) : undefined }
  }
  throw new Error(`${target.targetPlatform} 不支持 HTTP 外链离线下载`)
}
