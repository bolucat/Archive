import type { IDownloadUrl } from '../aliapi/models'
import { GetExpiresTime } from '../utils/utils'
import { apiBoxFileDetail, apiBoxFileList, buildBoxDownloadUrl, getBoxToken, mapBoxItemToAliModel } from './dirfilelist'
import { apiBoxShareCreate, apiBoxShareList } from './share'
import { apiBoxUploadBuffer } from './upload'
import { apiBoxCopyBatch, apiBoxDeleteBatch, apiBoxMkdir, apiBoxMoveBatch, apiBoxRename, apiBoxTrashListPage, apiBoxTrashPurge, apiBoxTrashRestore, type BoxItemType } from './filecmd'
import { apiBoxSearch, mapBoxSearchItems, parseBoxSearchId } from './search'
import { apiBoxFavoriteItems, apiBoxRecentItems } from './collections'

export const listBoxItems = async (userId: string, driveId: string, dirId: string, includeFiles: boolean) => {
  const isSearch = dirId.startsWith('search') || dirId.startsWith('box_search:')
  if (isSearch) {
    const { query } = parseBoxSearchId(dirId)
    if (!query) return { items: [], total: 0, error: '搜索关键字不能为空' }
    const items = mapBoxSearchItems(await apiBoxSearch(userId, query), driveId)
    const visibleItems = includeFiles ? items : items.filter(item => item.isDir)
    return { items: visibleItems, total: visibleItems.length }
  }

  const parentId = dirId === 'box_root' ? 'box_root' : dirId
  const items = (await apiBoxFileList(userId, parentId)).map(item => mapBoxItemToAliModel(item, driveId, parentId))
  const visibleItems = includeFiles ? items : items.filter(item => item.isDir)
  return { items: visibleItems, total: visibleItems.length }
}

export const listBoxSpecialItems = async (userId: string, dirId: string, maxItems = 0) => {
  if (dirId === 'recent') {
    const items = await apiBoxRecentItems(userId)
    return { items: maxItems > 0 ? items.slice(0, maxItems) : items, total: items.length }
  }
  if (dirId === 'favorite') {
    const items = await apiBoxFavoriteItems(userId)
    return { items: maxItems > 0 ? items.slice(0, maxItems) : items, total: items.length }
  }
  if (dirId !== 'trash') return undefined

  const items = [] as ReturnType<typeof mapBoxItemToAliModel>[]
  let marker = ''
  do {
    const page = await apiBoxTrashListPage(userId, marker)
    items.push(...page.items.map(item => mapBoxItemToAliModel(item, 'box', item.parent?.id || 'box_root')))
    marker = page.nextMarker
  } while (marker && (!maxItems || items.length < maxItems))
  const visibleItems = maxItems > 0 ? items.slice(0, maxItems) : items
  return { items: visibleItems, total: items.length }
}

export const listBoxShares = (userId: string) => apiBoxShareList(userId)

export const getBoxDownloadUrl = async (userId: string, driveId: string, fileId: string): Promise<IDownloadUrl | string> => {
  const token = await getBoxToken(userId)
  if (!token?.access_token) return '未登录 Box'
  const detail = await apiBoxFileDetail(userId, fileId, false)
  const url = buildBoxDownloadUrl(fileId, token.access_token)
  return { drive_id: driveId, file_id: fileId, expire_time: GetExpiresTime(url), url, size: Number(detail?.size || 0) }
}

export const getBoxFileInfo = async (userId: string, driveId: string, fileId: string) => {
  if (fileId === 'box_root') return { drive_id: driveId, file_id: fileId, parent_file_id: '', name: '根目录', type: 'folder', isDir: true }
  const detail = await apiBoxFileDetail(userId, fileId, false)
  if (!detail) return undefined
  const mapped = mapBoxItemToAliModel(detail, driveId, detail.parent?.id || 'box_root') as any
  mapped.type = mapped.isDir ? 'folder' : 'file'
  return mapped
}

export const createBoxShare = async (userId: string, driveId: string, fileIds: string[], shareName: string, isFolder: boolean, expiration: string, sharePwd: string) => {
  const result = await apiBoxShareCreate(userId, driveId, fileIds, shareName, isFolder, expiration, sharePwd)
  return result.error || result.item || '创建 Box 分享链接失败'
}

export const uploadBoxTextFile = async (userId: string, parentFileId: string, fileName: string, content: string, encType: string) => {
  if (encType) return 'Box 暂不支持加密新建文件'
  const result = await apiBoxUploadBuffer(userId, parentFileId, fileName, Buffer.from(content || '', 'utf-8'), 'refuse')
  return result.error || 'success'
}

export const createBoxFolder = (userId: string, parentFileId: string, name: string) => apiBoxMkdir(userId, parentFileId.includes('root') ? 'box_root' : parentFileId, name)

export const trashBoxFiles = (userId: string, fileIds: string[]) => apiBoxDeleteBatch(userId, fileIds)

export const renameBoxFiles = async (userId: string, fileIds: string[], names: string[]) => {
  const results: { file_id: string; parent_file_id: string; name: string; isDir: boolean }[] = []
  for (let i = 0; i < fileIds.length; i++) {
    const fileId = fileIds[i]
    const name = names[i] || ''
    if (!fileId || !name) continue
    const file = await apiBoxRename(userId, fileId, name)
    if (file) results.push({ file_id: file.file_id || fileId, parent_file_id: file.parent_file_id || '', name, isDir: file.isDir })
  }
  return results
}

export const moveBoxFiles = (userId: string, fileIds: string[], parentFileId: string, types: Record<string, BoxItemType | undefined> = {}) => apiBoxMoveBatch(userId, fileIds, parentFileId.includes('root') ? 'box_root' : parentFileId, types)
export const copyBoxFiles = (userId: string, fileIds: string[], parentFileId: string, types: Record<string, BoxItemType | undefined> = {}) => apiBoxCopyBatch(userId, fileIds, parentFileId.includes('root') ? 'box_root' : parentFileId, types)
export const cleanBoxTrashFiles = async (userId: string, fileIds: string[], types: Record<string, BoxItemType | undefined> = {}) => {
  const success: string[] = []
  for (const fileId of fileIds) {
    const type = types[fileId]
    if (type && await apiBoxTrashPurge(userId, fileId, type)) success.push(fileId)
  }
  return success
}
export const restoreBoxTrashFiles = async (userId: string, fileIds: string[], types: Record<string, BoxItemType | undefined> = {}) => {
  const success: string[] = []
  for (const fileId of fileIds) {
    const type = types[fileId]
    if (type && await apiBoxTrashRestore(userId, fileId, type)) success.push(fileId)
  }
  return success
}
