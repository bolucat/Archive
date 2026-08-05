import type { IDownloadUrl } from '../aliapi/models'
import { GetExpiresTime } from '../utils/utils'
import { apiGuangyaDownloadInfo, apiGuangyaFileDetail, apiGuangyaFileList, mapGuangyaFileToAliModel } from './dirfilelist'
import { apiGuangyaShareCreate, apiGuangyaShareList } from './share'
import { apiGuangyaUploadBuffer } from './upload'
import { apiGuangyaCopyBatch, apiGuangyaMkdir, apiGuangyaMoveBatch, apiGuangyaRename, apiGuangyaTrashBatch } from './filecmd'

export const listGuangyaItems = async (userId: string, driveId: string, dirId: string, includeFiles: boolean) => {
  const parentId = dirId === 'guangya_root' ? 'guangya_root' : dirId
  const mappedItems = (await apiGuangyaFileList(userId, parentId, 200)).map(item => mapGuangyaFileToAliModel(item, driveId, dirId))
  const visibleItems = includeFiles ? mappedItems : mappedItems.filter(item => item.isDir)
  return { items: visibleItems, total: visibleItems.length }
}

export const getGuangyaDownloadUrl = async (userId: string, driveId: string, fileId: string): Promise<IDownloadUrl | string> => {
  const result = await apiGuangyaDownloadInfo(userId, fileId)
  return result.error || { drive_id: driveId, file_id: fileId, expire_time: GetExpiresTime(result.url), url: result.url, size: Number(result.size || 0) }
}

export const getGuangyaFileInfo = async (userId: string, driveId: string, fileId: string) => {
  if (fileId === 'guangya_root' || fileId === '0' || fileId === '/') return { drive_id: driveId, file_id: 'guangya_root', parent_file_id: '', name: '根目录', type: 'folder', isDir: true }
  const detail = await apiGuangyaFileDetail(userId, fileId)
  if (!detail) return undefined
  const mapped = mapGuangyaFileToAliModel(detail, driveId, detail.parentId || detail.parentFileId || 'guangya_root') as any
  mapped.type = mapped.isDir ? 'folder' : 'file'
  return mapped
}

export const createGuangyaShare = async (userId: string, expiration: string, sharePwd: string, shareName: string, fileIds: string[]) => {
  const result = await apiGuangyaShareCreate(userId, expiration, sharePwd, shareName, fileIds)
  return result.error || result.item || '创建光鸭云盘分享链接失败'
}

export const listGuangyaShares = async (userId: string) => {
  const items = [] as Awaited<ReturnType<typeof apiGuangyaShareList>>['list']
  let page = 0
  do {
    const result = await apiGuangyaShareList(userId, page, 100)
    if (result.error) return { items, error: result.error }
    items.push(...result.list)
    page = Number(result.nextMarker)
    if (!result.nextMarker || Number.isNaN(page)) break
  } while (true)
  return { items, error: '' }
}

export const uploadGuangyaTextFile = async (userId: string, parentFileId: string, fileName: string, content: string, encType: string) => {
  if (encType) return '光鸭云盘暂不支持加密新建文件'
  const result = await apiGuangyaUploadBuffer(userId, parentFileId, fileName, Buffer.from(content || '', 'utf-8'))
  return result.error || 'success'
}

export const createGuangyaFolder = (userId: string, parentFileId: string, name: string) => apiGuangyaMkdir(userId, parentFileId.includes('root') ? 'guangya_root' : parentFileId, name)
export const trashGuangyaFiles = (userId: string, fileIds: string[]) => apiGuangyaTrashBatch(userId, fileIds)
export const moveGuangyaFiles = (userId: string, fileIds: string[], parentFileId: string) => apiGuangyaMoveBatch(userId, fileIds, parentFileId.includes('root') ? 'guangya_root' : parentFileId)
export const copyGuangyaFiles = (userId: string, fileIds: string[], parentFileId: string) => apiGuangyaCopyBatch(userId, fileIds, parentFileId.includes('root') ? 'guangya_root' : parentFileId)
export const renameGuangyaFiles = async (userId: string, fileIds: string[], names: string[]) => {
  const results: { file_id: string; parent_file_id: string; name: string; isDir: boolean }[] = []
  for (let index = 0; index < fileIds.length; index++) {
    const fileId = fileIds[index]
    const name = names[index] || ''
    if (!fileId || !name) continue
    const result = await apiGuangyaRename(userId, fileId, name)
    if (result.success) results.push({ file_id: fileId, parent_file_id: result.parent_file_id, name: result.name, isDir: result.isDir })
  }
  return results
}
