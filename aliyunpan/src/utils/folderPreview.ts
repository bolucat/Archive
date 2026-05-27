import { IAliGetFileModel } from '../aliapi/alimodels'
import AliDirFileList from '../aliapi/dirfilelist'
import {
  isAliyunUser,
  isBaiduUser,
  isBoxUser,
  isCloud123User,
  isDrive115User,
  isOneDriveUser,
  isPikPakUser
} from '../aliapi/utils'
import DebugLog from './debuglog'

export interface FolderPreviewParams {
  user_id: string
  drive_id: string
  file_id: string
  name?: string
  path?: string
}

const CACHE_TTL_MS = 5 * 60 * 1000
const MAX_PREVIEW_FILES = 12

interface CacheEntry {
  ts: number
  items: IAliGetFileModel[]
  promise?: Promise<IAliGetFileModel[]>
}

const previewCache = new Map<string, CacheEntry>()

const cacheKey = (p: FolderPreviewParams) => `${p.user_id}|${p.drive_id}|${p.file_id}`

const tryDynamicImport = async <T>(loader: () => Promise<T>): Promise<T | null> => {
  try {
    return await loader()
  } catch (e) {
    DebugLog.mSaveWarning('folderPreview dynamic import failed: ' + (e as Error).message)
    return null
  }
}

async function fetchFolderItemsRaw(p: FolderPreviewParams): Promise<IAliGetFileModel[]> {
  const { user_id, drive_id, file_id, name, path } = p
  if (!user_id || !drive_id || !file_id) return []

  try {
    if (isBaiduUser(user_id) || drive_id === 'baidu') {
      const mod = await tryDynamicImport(() => import('../cloudbaidu/dirfilelist'))
      if (!mod) return []
      // 百度网盘的目录列表 API 用路径而非 fs_id
      const dir = path && path.length ? path : (file_id === '/' ? '/' : path || '/')
      const list = await mod.apiBaiduFileList(user_id, dir, 'name', 0, MAX_PREVIEW_FILES, 0)
      return (list || []).map((item: any) => {
        const mapped = mod.mapBaiduFileToAliModel(item, drive_id, dir)
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isPikPakUser(user_id) || drive_id === 'pikpak') {
      const mod = await tryDynamicImport(() => import('../pikpak/dirfilelist'))
      if (!mod) return []
      const parentId = file_id === 'pikpak_root' ? '' : file_id
      const resp = await mod.apiPikPakFileList(user_id, parentId, MAX_PREVIEW_FILES)
      const items = resp?.items || []
      return items.map((item: any) => {
        const mapped = mod.mapPikPakFileToAliModel(item, drive_id, parentId)
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isCloud123User(user_id) || drive_id === 'cloud123') {
      const mod = await tryDynamicImport(() => import('../cloud123/dirfilelist'))
      if (!mod) return []
      const list = await mod.apiCloud123FileList(user_id, file_id, MAX_PREVIEW_FILES)
      return (list || []).map((item: any) => {
        const mapped = mod.mapCloud123FileToAliModel(item)
        mapped.drive_id = drive_id
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isDrive115User(user_id) || drive_id === 'drive115') {
      const mod = await tryDynamicImport(() => import('../cloud115/dirfilelist'))
      if (!mod) return []
      const list = await mod.apiDrive115FileList(user_id, file_id, MAX_PREVIEW_FILES, 0, true)
      return (list || []).map((item: any) => {
        const mapped = mod.mapDrive115FileToAliModel(item, drive_id)
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isOneDriveUser(user_id) || drive_id === 'onedrive') {
      const mod = await tryDynamicImport(() => import('../onedrive/dirfilelist'))
      if (!mod) return []
      const parentId = file_id === 'onedrive_root' ? 'onedrive_root' : file_id
      const list = await mod.apiOneDriveFileList(user_id, parentId)
      return (list || []).slice(0, MAX_PREVIEW_FILES).map((item: any) => {
        const mapped = mod.mapOneDriveItemToAliModel(item, drive_id, parentId)
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isBoxUser(user_id) || drive_id === 'box') {
      const mod = await tryDynamicImport(() => import('../box/dirfilelist'))
      if (!mod) return []
      const parentId = file_id === 'box_root' ? 'box_root' : file_id
      const list = await mod.apiBoxFileList(user_id, parentId, MAX_PREVIEW_FILES)
      return (list || []).map((item: any) => {
        const mapped = mod.mapBoxItemToAliModel(item, drive_id, parentId)
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (drive_id === 'dropbox') {
      const mod = await tryDynamicImport(() => import('../dropbox/dirfilelist'))
      if (!mod) return []
      const parentId = file_id === 'dropbox_root' ? 'dropbox_root' : file_id
      const list = await mod.apiDropboxFileList(user_id, parentId, MAX_PREVIEW_FILES)
      return (list || []).map((item: any) => {
        const mapped = mod.mapDropboxFileToAliModel(item, drive_id, parentId)
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isAliyunUser(user_id)) {
      const result = await AliDirFileList.ApiDirFileList(
        user_id,
        drive_id,
        file_id,
        name || '',
        'name asc',
        '',
        undefined,
        false
      )
      return (result?.items || []).slice(0, MAX_PREVIEW_FILES)
    }
  } catch (e) {
    DebugLog.mSaveWarning('folderPreview fetch failed: ' + (e as Error).message)
  }
  return []
}

export async function fetchFolderPreview(p: FolderPreviewParams): Promise<IAliGetFileModel[]> {
  const key = cacheKey(p)
  const now = Date.now()
  const cached = previewCache.get(key)
  if (cached) {
    if (cached.promise) return cached.promise
    if (now - cached.ts < CACHE_TTL_MS) return cached.items
  }
  const promise = fetchFolderItemsRaw(p).then((items) => {
    previewCache.set(key, { ts: Date.now(), items })
    return items
  }).catch((e) => {
    previewCache.delete(key)
    DebugLog.mSaveWarning('folderPreview promise rejected: ' + (e as Error).message)
    return [] as IAliGetFileModel[]
  })
  previewCache.set(key, { ts: now, items: [], promise })
  return promise
}

export function clearFolderPreviewCache(): void {
  previewCache.clear()
}

export const FOLDER_PREVIEW_MAX = MAX_PREVIEW_FILES
