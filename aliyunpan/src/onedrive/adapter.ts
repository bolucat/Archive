import type { IDownloadUrl } from '../aliapi/models'
import { GetExpiresTime } from '../utils/utils'
import { apiOneDriveFileDetail, apiOneDriveFileListPage, getOneDriveDownloadUrl, mapOneDriveItemToAliModel } from './dirfilelist'
import { apiOneDriveSearch, filterOneDriveSearchResults, mapOneDriveSearchItems, parseOneDriveSearchId } from './search'
import { apiOneDriveShareCreate } from './share'
import { apiOneDriveUploadBuffer } from './upload'
import { apiOneDriveCopyBatch, apiOneDriveDeleteBatch, apiOneDriveMkdir, apiOneDriveMoveBatch, apiOneDriveRename } from './filecmd'

export const getOneDriveProviderDownloadUrl = async (userId: string, driveId: string, fileId: string): Promise<IDownloadUrl | string> => {
  const detail = await apiOneDriveFileDetail(userId, fileId)
  const url = getOneDriveDownloadUrl(detail)
  return url ? { drive_id: driveId, file_id: fileId, expire_time: GetExpiresTime(url), url, size: Number(detail?.size || 0) } : '获取 OneDrive 下载地址失败'
}

export const getOneDriveFileInfo = async (userId: string, driveId: string, fileId: string) => {
  if (fileId === 'onedrive_root') return { drive_id: driveId, file_id: fileId, parent_file_id: '', name: '根目录', type: 'folder', isDir: true }
  const detail = await apiOneDriveFileDetail(userId, fileId)
  if (!detail) return undefined
  const mapped = mapOneDriveItemToAliModel(detail, driveId, detail.parentReference?.id || 'onedrive_root') as any
  mapped.type = mapped.isDir ? 'folder' : 'file'
  return mapped
}

export const createOneDriveShare = async (userId: string, driveId: string, fileIds: string[], shareName: string, expiration: string, sharePwd: string) => {
  if (sharePwd || expiration) return 'OneDrive 分享暂不支持提取码或有效期，请清空后重试'
  const result = await apiOneDriveShareCreate(userId, driveId, fileIds, shareName)
  return result.error || result.item || '创建 OneDrive 分享链接失败'
}

export const uploadOneDriveTextFile = async (userId: string, parentFileId: string, fileName: string, content: string, encType: string) => {
  if (encType) return 'OneDrive 暂不支持加密新建文件'
  const result = await apiOneDriveUploadBuffer(userId, parentFileId, fileName, Buffer.from(content || '', 'utf-8'))
  return result.error || 'success'
}

export const createOneDriveFolder = (userId: string, parentFileId: string, name: string) => apiOneDriveMkdir(userId, parentFileId.includes('root') ? 'onedrive_root' : parentFileId, name)

export const trashOneDriveFiles = (userId: string, fileIds: string[]) => apiOneDriveDeleteBatch(userId, fileIds)
export const moveOneDriveFiles = (userId: string, fileIds: string[], parentFileId: string) => apiOneDriveMoveBatch(userId, parentFileId.includes('root') ? 'onedrive_root' : parentFileId, fileIds)
export const copyOneDriveFiles = (userId: string, fileIds: string[], parentFileId: string, names: string[]) => apiOneDriveCopyBatch(userId, parentFileId.includes('root') ? 'onedrive_root' : parentFileId, fileIds.map((file_id, index) => ({ file_id, name: names[index] || '' })))
export const renameOneDriveFiles = async (userId: string, fileIds: string[], names: string[]) => {
  const results: { file_id: string; parent_file_id: string; name: string; isDir: boolean }[] = []
  for (let index = 0; index < fileIds.length; index++) {
    const fileId = fileIds[index]
    const name = names[index] || ''
    if (!fileId || !name) continue
    const result = await apiOneDriveRename(userId, fileId, name)
    if (!result.error) results.push({ file_id: result.file_id || fileId, parent_file_id: '', name, isDir: true })
  }
  return results
}

export const listOneDriveItems = async (userId: string, driveId: string, dirId: string, includeFiles: boolean, nextLink = '') => {
  if (dirId.startsWith('search')) {
    const filters = parseOneDriveSearchId(dirId)
    if (!filters.query) return { items: [], total: 0, error: '搜索关键字不能为空' }
    const items = filterOneDriveSearchResults(mapOneDriveSearchItems(await apiOneDriveSearch(userId, filters.query), driveId), filters)
    const visible = includeFiles ? items : items.filter(item => item.isDir)
    return { items: visible, total: visible.length }
  }
  const parentId = dirId === 'onedrive_root' ? 'onedrive_root' : dirId
  const page = await apiOneDriveFileListPage(userId, parentId, nextLink, false, 200)
  const items = page.items.map(item => mapOneDriveItemToAliModel(item, driveId, parentId))
  const visible = includeFiles ? items : items.filter(item => item.isDir)
  return { items: visible, total: visible.length, nextCursor: includeFiles ? page.nextLink : '' }
}
