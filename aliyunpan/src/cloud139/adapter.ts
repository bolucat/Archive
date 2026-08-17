import type { IDownloadUrl } from '../aliapi/models'
import { GetExpiresTime } from '../utils/utils'
import { apiCloud139DownloadInfo, apiCloud139FileDetail, apiCloud139FileListPage, cloud139DownloadHeaders, mapCloud139FileToAliModel } from './dirfilelist'
import { apiCloud139CopyBatch, apiCloud139Mkdir, apiCloud139MoveBatch, apiCloud139Rename, apiCloud139TrashBatch } from './filecmd'

export const listCloud139Items = async (userId: string, driveId: string, dirId: string, includeFiles: boolean, cursor = '') => {
  const parentId = dirId === 'cloud139_root' ? '/' : dirId
  const page = await apiCloud139FileListPage(userId, parentId, 200, cursor)
  const mappedItems = page.items.map(item => mapCloud139FileToAliModel(item, driveId, dirId))
  const visibleItems = includeFiles ? mappedItems : mappedItems.filter(item => item.isDir)
  return { items: visibleItems, total: mappedItems.length, nextCursor: page.nextCursor }
}

export const getCloud139DownloadUrl = async (userId: string, driveId: string, fileId: string): Promise<IDownloadUrl | string> => {
  const result = await apiCloud139DownloadInfo(userId, fileId)
  return result.error || { drive_id: driveId, file_id: fileId, expire_time: GetExpiresTime(result.url), url: result.url, size: Number(result.size || 0), headers: cloud139DownloadHeaders() }
}

export const getCloud139FileInfo = async (userId: string, driveId: string, fileId: string) => {
  if (fileId === 'cloud139_root' || fileId === '/' || fileId === '0') return { drive_id: driveId, file_id: 'cloud139_root', parent_file_id: '', name: '根目录', type: 'folder', isDir: true }
  const detail = await apiCloud139FileDetail(userId, fileId)
  if (!detail) return undefined
  const mapped = mapCloud139FileToAliModel(detail, driveId, detail.parentFileId || detail.parentCatalogId || 'cloud139_root') as any
  mapped.type = mapped.isDir ? 'folder' : 'file'
  return mapped
}

export const createCloud139Folder = (userId: string, parentFileId: string, name: string) => apiCloud139Mkdir(userId, parentFileId.includes('root') ? 'cloud139_root' : parentFileId, name)
export const trashCloud139Files = (userId: string, fileIds: string[]) => apiCloud139TrashBatch(userId, fileIds)
export const moveCloud139Files = (userId: string, fileIds: string[], parentFileId: string) => apiCloud139MoveBatch(userId, fileIds, parentFileId.includes('root') ? 'cloud139_root' : parentFileId)
export const copyCloud139Files = (userId: string, fileIds: string[], parentFileId: string) => apiCloud139CopyBatch(userId, fileIds, parentFileId.includes('root') ? 'cloud139_root' : parentFileId)
export const renameCloud139Files = async (userId: string, fileIds: string[], names: string[]) => {
  const results: { file_id: string; parent_file_id: string; name: string; isDir: boolean }[] = []
  for (let index = 0; index < fileIds.length; index++) {
    const fileId = fileIds[index]
    const name = names[index] || ''
    if (!fileId || !name) continue
    const result = await apiCloud139Rename(userId, fileId, name)
    if (result.success) results.push({ file_id: fileId, parent_file_id: result.parent_file_id, name: result.name, isDir: result.isDir })
  }
  return results
}
