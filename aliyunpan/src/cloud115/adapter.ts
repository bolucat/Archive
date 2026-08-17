import type { IDownloadUrl, IVideoPreviewUrl } from '../aliapi/models'
import { GetExpiresTime } from '../utils/utils'
import { apiDrive115FileDetail, apiDrive115Mkdir } from './filecmd'
import { apiDrive115DownUrl } from './download'
import { apiDrive115FileList, apiDrive115Search, mapDrive115DetailToAliModel, mapDrive115FileToAliModel, mapDrive115SearchToAliModel, mapDrive115TrashToAliModel } from './dirfilelist'
import { apiDrive115VideoHistoryUpdate, apiDrive115VideoPlay, apiDrive115VideoSubtitle, getDrive115PickCode } from './video'
import { apiDrive115CopyBatch } from './copy'
import { apiDrive115MoveBatch } from './move'
import { apiDrive115Rename } from './rename'
import { apiDrive115TrashBatch, apiDrive115TrashDelete, apiDrive115TrashRestore } from './trash'
import { apiDrive115TrashList } from './trash'

export const listDrive115Items = async (userId: string, driveId: string, dirId: string, includeFiles: boolean, offset = 0) => {
  if (dirId === 'trash') {
    const { items, total } = await apiDrive115TrashList(userId, 200, 0)
    const mappedItems = items.map(item => mapDrive115TrashToAliModel(item, driveId))
    return { items: mappedItems, total: total || mappedItems.length }
  }

  if (dirId.startsWith('search')) {
    const searchValue = dirId.substring('search'.length).trim()
    if (!searchValue) return { items: [], total: 0 }
    const { items, total } = await apiDrive115Search(userId, searchValue, 200, 0)
    const mappedItems = items.map(item => mapDrive115SearchToAliModel(item, driveId))
    return { items: mappedItems, total: total || mappedItems.length }
  }

  const parentId = dirId === 'drive115_root' ? 0 : dirId
  const pageSize = 200
  const items = (await apiDrive115FileList(userId, parentId, pageSize, offset, true)).map(item => mapDrive115FileToAliModel(item, driveId))
  const visibleItems = includeFiles ? items : items.filter(item => item.isDir)
  return { items: visibleItems, total: visibleItems.length, nextCursor: includeFiles && items.length === pageSize ? String(offset + pageSize) : '' }
}

export const getDrive115DownloadUrl = async (userId: string, driveId: string, fileId: string): Promise<IDownloadUrl | string> => {
  const detail = await apiDrive115FileDetail(userId, fileId)
  if (!detail) return '获取文件详情失败'
  const result = await apiDrive115DownUrl(userId, detail.pick_code)
  return typeof result === 'string' ? result : { drive_id: driveId, file_id: fileId, expire_time: GetExpiresTime(result.url), url: result.url, size: result.size || detail.size || 0, headers: result.headers }
}

export const getDrive115FileInfo = async (userId: string, driveId: string, fileId: string) => {
  const detail = await apiDrive115FileDetail(userId, fileId)
  if (!detail) return undefined
  const mapped = mapDrive115DetailToAliModel(detail, driveId) as any
  mapped.type = mapped.isDir ? 'folder' : 'file'
  mapped.pick_code = detail.pick_code
  return mapped
}

export const getDrive115VideoPreview = async (userId: string, driveId: string, fileId: string): Promise<IVideoPreviewUrl | string> => {
  const meta = await getDrive115PickCode(userId, fileId)
  if (!meta?.pick_code) return meta?.error || '获取文件详情失败'
  const playInfo = await apiDrive115VideoPlay(userId, meta.pick_code)
  if (typeof playInfo === 'string') return playInfo
  const subtitles = await apiDrive115VideoSubtitle(userId, meta.pick_code)
  const detectVideoType = (url: string, fallback = '') => {
    const lower = String(url || '').split('?')[0].split('#')[0].toLowerCase()
    if (lower.endsWith('.m3u8')) return 'm3u8'
    if (lower.endsWith('.mpd')) return 'mpd'
    if (lower.endsWith('.ts')) return 'ts'
    return fallback
  }
  const defLabel = (def: string) => {
    switch (def) {
      case '1': return '标清'
      case '2': return '高清'
      case '3': return '超清'
      case '4': return '1080P'
      case '5': return '4K'
      case '100': return '原画'
      default: return def ? `清晰度${def}` : '清晰度'
    }
  }
  const data: IVideoPreviewUrl = { drive_id: driveId, file_id: fileId, size: 0, expire_time: 0, width: 0, height: 0, duration: 0, qualities: [], subtitles }
  data.qualities = (playInfo.video_url || [])
    .filter(item => item && item.url)
    .map(item => {
      const def = String(item.definition ?? item.definition_n ?? '')
      const label = item.title || defLabel(def)
      return { html: label, quality: def, height: Number(item.height || 0), width: Number(item.width || 0), label, value: label, url: item.url, type: detectVideoType(item.url, def && def !== '100' ? 'm3u8' : ''), headers: item.headers }
    })
    .sort((a, b) => (b.width || 0) - (a.width || 0))
  const userDef = playInfo.user_def ? String(playInfo.user_def) : ''
  const selectedIndex = data.qualities.findIndex(quality => quality.quality === userDef)
  if (selectedIndex > 0) data.qualities.unshift(data.qualities.splice(selectedIndex, 1)[0])
  if (data.qualities.length > 0) {
    const first = data.qualities[0]
    data.width = first.width || 0
    data.height = first.height || 0
    data.expire_time = GetExpiresTime(first.url)
    if (first.headers) data.headers = first.headers
  }
  data.duration = Math.floor(Number(playInfo.play_long || meta.play_long || 0))
  return data
}

export const updateDrive115VideoHistory = async (userId: string, fileId: string, playCursor: number) => {
  const meta = await getDrive115PickCode(userId, fileId)
  if (!meta?.pick_code) return
  const playLong = Number(meta.play_long || 0)
  await apiDrive115VideoHistoryUpdate(userId, meta.pick_code, playCursor, playLong > 0 && playCursor >= playLong - 10 ? 1 : 0)
}

export const createDrive115Folder = (userId: string, parentFileId: string, name: string) => apiDrive115Mkdir(userId, parentFileId.includes('root') ? '0' : parentFileId, name)
export const trashDrive115Files = (userId: string, fileIds: string[]) => apiDrive115TrashBatch(userId, fileIds)
export const deleteDrive115TrashFiles = (userId: string, fileIds: string[]) => apiDrive115TrashDelete(userId, fileIds)
export const restoreDrive115TrashFiles = (userId: string, fileIds: string[]) => apiDrive115TrashRestore(userId, fileIds)
export const moveDrive115Files = (userId: string, fileIds: string[], parentFileId: string) => apiDrive115MoveBatch(userId, fileIds, parentFileId.includes('root') ? '0' : parentFileId)
export const copyDrive115Files = (userId: string, fileIds: string[], parentFileId: string) => apiDrive115CopyBatch(userId, fileIds, parentFileId.includes('root') ? '0' : parentFileId)
export const renameDrive115Files = async (userId: string, fileIds: string[], names: string[]) => {
  const results: { file_id: string; parent_file_id: string; name: string; isDir: boolean }[] = []
  for (let index = 0; index < fileIds.length; index++) {
    const fileId = fileIds[index]
    const name = names[index] || ''
    if (!fileId || !name) continue
    const result = await apiDrive115Rename(userId, fileId, name)
    if (result.success) results.push({ file_id: fileId, parent_file_id: '', name: result.name, isDir: true })
  }
  return results
}
