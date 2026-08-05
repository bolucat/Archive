import type { IDownloadUrl } from '../aliapi/models'
import { GetExpiresTime } from '../utils/utils'
import { apiCloud189DownloadInfo, apiCloud189FileDetail, apiCloud189FileList, cloud189DownloadHeaders, mapCloud189FileToAliModel } from './dirfilelist'
import { apiCloud189CopyBatch, apiCloud189Mkdir, apiCloud189MoveBatch, apiCloud189Rename, apiCloud189TrashBatch } from './filecmd'

export const listCloud189Items = async (userId: string, driveId: string, dirId: string, includeFiles: boolean) => {
  const parentId = dirId === 'cloud189_root' ? '-11' : dirId
  const mappedItems = (await apiCloud189FileList(userId, parentId, 200)).map(item => mapCloud189FileToAliModel(item, driveId, dirId))
  const visibleItems = includeFiles ? mappedItems : mappedItems.filter(item => item.isDir)
  return { items: visibleItems, total: mappedItems.length }
}

export const getCloud189DownloadUrl = async (userId: string, driveId: string, fileId: string): Promise<IDownloadUrl | string> => {
  const result = await apiCloud189DownloadInfo(userId, fileId)
  return result.error || { drive_id: driveId, file_id: fileId, expire_time: GetExpiresTime(result.url), url: result.url, size: Number(result.size || 0), headers: cloud189DownloadHeaders() }
}

export const getCloud189FileInfo = async (userId: string, driveId: string, fileId: string) => {
  if (fileId === 'cloud189_root' || fileId === '-11' || fileId === '0') return { drive_id: driveId, file_id: 'cloud189_root', parent_file_id: '', name: '根目录', type: 'folder', isDir: true }
  const detail = await apiCloud189FileDetail(userId, fileId)
  if (!detail) return undefined
  const mapped = mapCloud189FileToAliModel(detail, driveId, detail.parentId || detail.parentFolderId || 'cloud189_root') as any
  mapped.type = mapped.isDir ? 'folder' : 'file'
  return mapped
}

export const createCloud189Folder = (userId: string, parentFileId: string, name: string) => apiCloud189Mkdir(userId, parentFileId.includes('root') ? 'cloud189_root' : parentFileId, name)
export const trashCloud189Files = (userId: string, fileIds: string[]) => apiCloud189TrashBatch(userId, fileIds)
export const moveCloud189Files = (userId: string, fileIds: string[], parentFileId: string) => apiCloud189MoveBatch(userId, fileIds, parentFileId.includes('root') ? 'cloud189_root' : parentFileId)
export const copyCloud189Files = (userId: string, fileIds: string[], parentFileId: string) => apiCloud189CopyBatch(userId, fileIds, parentFileId.includes('root') ? 'cloud189_root' : parentFileId)
export const renameCloud189Files = async (userId: string, fileIds: string[], names: string[]) => {
  const results: { file_id: string; parent_file_id: string; name: string; isDir: boolean }[] = []
  for (let index = 0; index < fileIds.length; index++) {
    const fileId = fileIds[index]
    const name = names[index] || ''
    if (!fileId || !name) continue
    const result = await apiCloud189Rename(userId, fileId, name)
    if (result.success) results.push({ file_id: fileId, parent_file_id: result.parent_file_id, name: result.name, isDir: result.isDir })
  }
  return results
}
