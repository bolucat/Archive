import type { IDownloadUrl } from '../aliapi/models'
import type { IAliShareItem } from '../aliapi/alimodels'
import { humanExpiration } from '../utils/format'
import { GetExpiresTime } from '../utils/utils'
import { apiDropboxFileDetail, apiDropboxFileList, apiDropboxTemporaryLink, mapDropboxFileToAliModel, resolveDropboxParentIdFromPath } from './dirfilelist'
import { apiDropboxSearch, filterDropboxSearchResults, parseDropboxSearchId } from './search'
import { apiDropboxThumbnails } from './thumbnail'
import { apiDropboxListSharedLinks, apiDropboxShareCreate, mapDropboxSharedLinkToAliShareItem } from './share'
import { apiDropboxUploadBuffer } from './upload'
import { apiDropboxCopyBatch, apiDropboxDeleteBatch, apiDropboxMkdir, apiDropboxMoveBatch, apiDropboxRename } from './filecmd'

export const getDropboxDownloadUrl = async (userId: string, driveId: string, fileId: string): Promise<IDownloadUrl | string> => {
  const result = await apiDropboxTemporaryLink(userId, fileId)
  return result.error || { drive_id: driveId, file_id: fileId, expire_time: GetExpiresTime(result.url), url: result.url, size: Number(result.metadata?.size || 0) }
}

export const getDropboxFileInfo = async (userId: string, driveId: string, fileId: string) => {
  if (fileId === 'dropbox_root') return { drive_id: driveId, file_id: fileId, parent_file_id: '', name: '根目录', type: 'folder', isDir: true }
  const detail = await apiDropboxFileDetail(userId, fileId)
  if (!detail) return undefined
  const mapped = mapDropboxFileToAliModel(detail, driveId, resolveDropboxParentIdFromPath(detail.path_display || detail.path_lower)) as any
  mapped.type = mapped.isDir ? 'folder' : 'file'
  return mapped
}

export const createDropboxShare = async (userId: string, driveId: string, fileIds: string[], expiration: string, sharePwd: string, shareName: string) => {
  const result = await apiDropboxShareCreate(userId, driveId, fileIds, expiration, sharePwd, shareName)
  return result.error || result.item || '创建 Dropbox 分享链接失败'
}

export const uploadDropboxTextFile = async (accessToken: string, parentFileId: string, fileName: string, content: string, encType: string, parentDescription = '') => {
  if (encType) return 'Dropbox 暂不支持加密新建文件'
  const result = await apiDropboxUploadBuffer(accessToken, parentFileId, fileName, Buffer.from(content || '', 'utf-8'), 'refuse', parentDescription)
  return result.error || 'success'
}

export const createDropboxFolder = (userId: string, parentFileId: string, name: string, parentDescription = '') => apiDropboxMkdir(userId, parentFileId.includes('root') ? 'dropbox_root' : parentFileId, name, parentDescription)

export const trashDropboxFiles = (userId: string, fileIds: string[]) => apiDropboxDeleteBatch(userId, fileIds)

export const renameDropboxFiles = async (userId: string, fileIds: string[], names: string[]) => {
  const results: { file_id: string; parent_file_id: string; name: string; isDir: boolean }[] = []
  for (let i = 0; i < fileIds.length; i++) {
    const fileId = fileIds[i]
    const name = names[i] || ''
    if (!fileId || !name) continue
    const file = await apiDropboxRename(userId, fileId, name)
    if (file.success) results.push({ file_id: file.file_id, parent_file_id: file.parent_file_id, name: file.name, isDir: file.isDir })
  }
  return results
}

export const moveDropboxFiles = (userId: string, fileIds: string[], parentFileId: string, parentDescription = '') => apiDropboxMoveBatch(userId, fileIds, parentFileId.includes('root') ? 'dropbox_root' : parentFileId, parentDescription)
export const copyDropboxFiles = (userId: string, fileIds: string[], parentFileId: string, parentDescription = '') => apiDropboxCopyBatch(userId, fileIds, parentFileId.includes('root') ? 'dropbox_root' : parentFileId, parentDescription)

export const listDropboxItems = async (userId: string, driveId: string, dirId: string, includeFiles: boolean) => {
  const hydrateThumbnails = async (items: any[]) => {
    const candidates = items.filter(item => !item.isDir && !item.thumbnail && item.category === 'image').slice(0, 40)
    const thumbnails = await apiDropboxThumbnails(userId, candidates.map(item => item.file_id)).catch(() => new Map<string, string>())
    candidates.forEach(item => {
      const thumbnail = thumbnails.get(item.file_id)
      if (thumbnail) item.thumbnail = thumbnail
    })
    return items
  }
  if (dirId.startsWith('search')) {
    const filters = parseDropboxSearchId(dirId)
    if (!filters.query) return { items: [], total: 0, error: '搜索关键字不能为空' }
    const items = (await apiDropboxSearch(userId, filters.query, 1000)).map(item => {
      const parentId = item.path_display ? item.path_display.split('/').slice(0, -1).join('/') : 'dropbox_root'
      return mapDropboxFileToAliModel(item, driveId, parentId || 'dropbox_root')
    })
    const filtered = filterDropboxSearchResults(items, filters)
    const visible = includeFiles ? filtered : filtered.filter(item => item.isDir)
    return { items: await hydrateThumbnails(visible), total: visible.length }
  }
  const parentId = dirId === 'dropbox_root' ? 'dropbox_root' : dirId
  const items = (await apiDropboxFileList(userId, parentId, 500)).map(item => mapDropboxFileToAliModel(item, driveId, parentId))
  const visible = includeFiles ? items : items.filter(item => item.isDir)
  return { items: await hydrateThumbnails(visible), total: visible.length }
}

export const listDropboxShares = async (userId: string): Promise<{ items: IAliShareItem[]; error: string }> => {
  const now = Date.now()
  const items = (await apiDropboxListSharedLinks(userId, '')).map(link => {
    const item = mapDropboxSharedLinkToAliShareItem(link, 'dropbox', link.path_lower ? [link.path_lower] : [], link.name || '', '')
    item.created_at = ''
    item.share_msg = humanExpiration(item.expiration, now)
    return item
  })
  return { items, error: '' }
}
