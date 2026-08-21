import type { IDownloadUrl } from '../aliapi/models'
import usePanTreeStore from '../pan/pantreestore'
import TreeStore from '../store/treestore'
import { GetExpiresTime } from '../utils/utils'
import { getBaiduToken } from './auth'
import { apiBaiduFileMetas, mapBaiduMetaToAliFileItem } from './filecmd'
import { apiBaiduCopy, apiBaiduDelete, apiBaiduMove, apiBaiduRename } from './filemanager'
import { apiBaiduCreateDir, buildBaiduUploadPath } from './upload'
import { apiBaiduFileListPage, apiBaiduSearch, mapBaiduFileToAliModel } from './dirfilelist'

const resolveCachedBaiduDirPath = (driveId: string, dirId: string): string => {
  if (!dirId || dirId === 'baidu_root') return '/'
  if (dirId.startsWith('/')) return dirId
  const dir = TreeStore.GetDir(driveId, dirId)
  if (dir?.path) return dir.path
  const selectedDir: any = usePanTreeStore().selectDir
  if (selectedDir?.file_id === dirId && selectedDir?.path) return selectedDir.path
  const description = dir?.description || selectedDir?.description || ''
  const match = /baidu_path:([^;]+)/.exec(description)
  return match?.[1] || ''
}

export const resolveBaiduDirectoryPath = async (userId: string, driveId: string, dirId: string): Promise<string> => {
  const cachedPath = resolveCachedBaiduDirPath(driveId, dirId)
  if (cachedPath) return cachedPath
  const fsId = Number(dirId.match(/baidu_fsid:(\d+)/)?.[1] || dirId)
  if (!Number.isFinite(fsId)) return ''
  const metas = await apiBaiduFileMetas(userId, [fsId], 0)
  return metas?.[0]?.path || ''
}

export const listBaiduItems = async (userId: string, driveId: string, dirId: string, includeFiles: boolean, start = 0) => {
  const isSearch = dirId.startsWith('search')
  const dirPath = isSearch ? '/' : await resolveBaiduDirectoryPath(userId, driveId, dirId)
  if (!dirPath) return { items: [], total: 0, error: '无法确定百度网盘目录路径，请刷新目录后重试' }
  const parentPath = isSearch ? '/' : dirPath
  const [sortKey, sortDirection] = TreeStore.GetDirOrder(driveId, dirId).split(' ')
  const order = sortKey === 'updated_at' ? 'time' : sortKey === 'size' ? 'size' : 'name'
  const desc = sortDirection === 'desc' ? 1 : 0
  const result = isSearch
    ? { items: await apiBaiduSearch(userId, dirId.substring('search'.length).trim(), '/', true), hasMore: false }
    : await apiBaiduFileListPage(userId, dirPath, order, start, 1000, desc)
  const list = result.items
  const mappedItems = list.map(item => mapBaiduFileToAliModel(item, driveId, parentPath))
  const visibleItems = includeFiles ? mappedItems : mappedItems.filter(item => item.isDir)
  return { items: visibleItems, total: visibleItems.length, nextCursor: includeFiles && result.hasMore ? String(start + 1000) : '' }
}

export const getBaiduDownloadUrl = async (userId: string, driveId: string, fileId: string): Promise<IDownloadUrl | string> => {
  const fsId = Number(fileId.match(/baidu_fsid:(\d+)/)?.[1] || fileId)
  if (!Number.isFinite(fsId)) return '获取下载地址失败'
  const metas = await apiBaiduFileMetas(userId, [fsId], 1)
  if (!metas?.[0]?.dlink) return '获取下载地址失败'
  let url = metas[0].dlink
  const token = await getBaiduToken(userId)
  if (token?.access_token && !url.includes('access_token=')) url += (url.includes('?') ? '&' : '?') + `access_token=${encodeURIComponent(token.access_token)}`
  return {
    drive_id: driveId,
    file_id: fileId,
    expire_time: GetExpiresTime(url),
    url,
    size: Number(metas[0].size || 0),
    headers: {
      'User-Agent': 'pan.baidu.com',
      Referer: 'https://pan.baidu.com/'
    }
  }
}

export const getBaiduFileInfo = async (userId: string, driveId: string, fileId: string) => {
  if (fileId === 'baidu_root' || fileId === '/') return { drive_id: driveId, file_id: fileId, parent_file_id: '', name: '根目录', type: 'folder', isDir: true }
  const fsId = Number(fileId.match(/baidu_fsid:(\d+)/)?.[1] || fileId)
  if (!Number.isFinite(fsId)) return undefined
  const metas = await apiBaiduFileMetas(userId, [fsId], 1)
  if (!metas?.[0]) return undefined
  const mapped = mapBaiduMetaToAliFileItem(metas[0], driveId, fileId) as any
  mapped.type = mapped.isDir ? 'folder' : 'file'
  return mapped
}

export const createBaiduFolder = async (userId: string, parentPath: string, name: string, checkNameMode: string) => {
  const result = await apiBaiduCreateDir(userId, buildBaiduUploadPath(parentPath.includes('root') ? '/' : parentPath, name), checkNameMode === 'auto_rename' ? 1 : 0)
  return { file_id: result.path, error: result.error }
}
export const trashBaiduFiles = (userId: string, paths: string[]) => apiBaiduDelete(userId, paths)
export const moveBaiduFiles = (userId: string, paths: string[], parentPath: string) => apiBaiduMove(userId, paths, parentPath)
export const copyBaiduFiles = (userId: string, paths: string[], parentPath: string) => apiBaiduCopy(userId, paths, parentPath)
export const renameBaiduFiles = async (userId: string, fileIds: string[], paths: string[], names: string[]) => {
  const results: { file_id: string; parent_file_id: string; name: string; isDir: boolean }[] = []
  for (let index = 0; index < fileIds.length; index++) {
    const fileId = fileIds[index]
    const path = paths[index] || fileId
    const name = names[index] || ''
    if (!path || !name) continue
    if ((await apiBaiduRename(userId, path, name)).length) results.push({ file_id: fileId, parent_file_id: '', name, isDir: true })
  }
  return results
}
