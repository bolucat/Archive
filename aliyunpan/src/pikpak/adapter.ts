import type { IAliShareItem } from '../aliapi/alimodels'
import type { IDownloadUrl } from '../aliapi/models'
import { humanExpiration } from '../utils/format'
import { GetExpiresTime } from '../utils/utils'
import { apiPikPakDownloadInfo, apiPikPakFileDetail, apiPikPakFileList, mapPikPakFileToAliModel } from './dirfilelist'
import { apiPikPakShareCreate, apiPikPakShareList, encodePikPakShareId } from './share'
import { apiPikPakCopyBatch, apiPikPakMkdir, apiPikPakMoveBatch, apiPikPakRename, apiPikPakTrashBatch, apiPikPakTrashDelete, apiPikPakTrashRestore } from './filecmd'

export const listPikPakItems = async (userId: string, driveId: string, dirId: string, includeFiles: boolean, pageToken = '') => {
  if (dirId.startsWith('search')) return { items: [], total: 0, error: 'PikPak 暂不支持搜索' }
  const isTrash = dirId === 'trash'
  const parentId = dirId === 'pikpak_root' || isTrash ? 'pikpak_root' : dirId
  const { items, nextPageToken } = await apiPikPakFileList(userId, parentId, 100, pageToken, isTrash)
  const mappedItems = items.map(item => mapPikPakFileToAliModel(item, driveId, parentId))
  const visibleItems = includeFiles ? mappedItems : mappedItems.filter(item => item.isDir)
  return { items: visibleItems, total: visibleItems.length, nextCursor: includeFiles ? nextPageToken : '' }
}

export const listPikPakShares = async (userId: string): Promise<{ items: IAliShareItem[]; error: string }> => {
  const result = await apiPikPakShareList(userId)
  return { error: result.error, items: result.list.map(item => ({ created_at: item.createdAt || '', creator: '', description: '', display_name: '', display_label: '', download_count: item.downloadCount, drive_id: 'pikpak', expiration: item.expiration, expired: item.expired, file_id: '', file_id_list: [], icon: 'iconwenjian', preview_count: item.previewCount, save_count: item.saveCount, share_id: encodePikPakShareId(item.shareId), share_msg: item.expired ? '过期失效' : humanExpiration(item.expiration), full_share_msg: '', share_name: item.title, share_policy: '', share_pwd: item.passCode, share_url: item.shareUrl, status: item.expired ? 'expired' : '', updated_at: '', is_share_saved: false, share_saved: '' })) }
}

export const getPikPakDownloadUrl = async (userId: string, driveId: string, fileId: string): Promise<IDownloadUrl | string> => {
  const result = await apiPikPakDownloadInfo(userId, fileId)
  const url = result.streamUrl || result.downloadUrl
  return result.error || !url ? result.error || '获取下载地址失败' : { drive_id: driveId, file_id: fileId, expire_time: GetExpiresTime(url), url, size: Number(result.item?.size || 0) }
}

export const getPikPakFileInfo = async (userId: string, driveId: string, fileId: string) => {
  if (fileId === 'pikpak_root') return { drive_id: driveId, file_id: fileId, parent_file_id: '', name: '根目录', type: 'folder', isDir: true }
  const detail = await apiPikPakFileDetail(userId, fileId)
  if (!detail) return undefined
  const mapped = mapPikPakFileToAliModel(detail, driveId, detail.parent_id || 'pikpak_root') as any
  mapped.type = mapped.isDir ? 'folder' : 'file'
  return mapped
}

export const createPikPakShare = async (userId: string, driveId: string, fileIds: string[], shareName: string, expiration: string, sharePwd: string): Promise<string | IAliShareItem> => {
  const result = await apiPikPakShareCreate(userId, fileIds, !!sharePwd, expiration)
  if (result.error) return result.error
  return { created_at: '', creator: '', description: '', display_name: '', display_label: '', download_count: 0, drive_id: driveId, expiration, expired: false, file_id: '', file_id_list: fileIds, icon: 'iconwenjian', preview_count: 0, save_count: 0, share_id: result.shareId, share_msg: humanExpiration(expiration), full_share_msg: '', share_name: shareName || '分享链接', share_policy: '', share_pwd: result.passCode || sharePwd, share_url: result.shareUrl, status: '', updated_at: '', is_share_saved: false, share_saved: '' }
}

export const createPikPakFolder = (userId: string, parentFileId: string, name: string) => apiPikPakMkdir(userId, parentFileId.includes('root') ? 'pikpak_root' : parentFileId, name)
export const trashPikPakFiles = (userId: string, fileIds: string[]) => apiPikPakTrashBatch(userId, fileIds)
export const deletePikPakTrashFiles = (userId: string, fileIds: string[]) => apiPikPakTrashDelete(userId, fileIds)
export const restorePikPakTrashFiles = (userId: string, fileIds: string[]) => apiPikPakTrashRestore(userId, fileIds)
export const movePikPakFiles = (userId: string, fileIds: string[], parentFileId: string) => apiPikPakMoveBatch(userId, fileIds, parentFileId.includes('root') ? 'pikpak_root' : parentFileId)
export const copyPikPakFiles = (userId: string, fileIds: string[], parentFileId: string) => apiPikPakCopyBatch(userId, fileIds, parentFileId.includes('root') ? 'pikpak_root' : parentFileId)
export const renamePikPakFiles = async (userId: string, fileIds: string[], names: string[]) => {
  const results: { file_id: string; parent_file_id: string; name: string; isDir: boolean }[] = []
  for (let index = 0; index < fileIds.length; index++) {
    const fileId = fileIds[index]
    const name = names[index] || ''
    if (!fileId || !name) continue
    const result = await apiPikPakRename(userId, fileId, name)
    if (result.success) results.push({ file_id: fileId, parent_file_id: result.parent_file_id, name: result.name, isDir: result.isDir })
  }
  return results
}
