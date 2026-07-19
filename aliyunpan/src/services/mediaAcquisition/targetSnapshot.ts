import type { MediaAcquisitionFileSnapshot, MediaAcquisitionTarget } from '@shared/types/mediaAcquisition'
import AliTrash from '../../aliapi/trash'
import { apiCloud123FileListPage } from '../../cloud123/dirfilelist'
import { apiDrive115FileList } from '../../cloud115/dirfilelist'
import { apiGuangyaFileList, getGuangyaFileId, getGuangyaFileName, isGuangyaDir } from '../../guangya/dirfilelist'
import { apiPikPakFileList } from '../../pikpak/dirfilelist'
import { apiQuarkFileList } from '../../quark/dirfilelist'
import { normalizeMediaAcquisitionPlatform, normalizeMediaAcquisitionRootFolder } from './capabilities'
import { buildMediaAcquisitionLeafPath } from './organizerPolicy'
import { extractObtainedEpisodeNumbers } from './tracking'
export { newMediaAcquisitionFiles } from './snapshotDiff'

export type MediaAcquisitionDirectoryEntry = MediaAcquisitionFileSnapshot & { isDir: boolean }

const VIDEO_EXT = /\.(mp4|mkv|avi|mov|wmv|flv|webm|m4v|mpg|mpeg|3gp|rmvb|ts|m2ts|mts|vob)$/i

export async function listMediaAcquisitionTargetFiles(target: MediaAcquisitionTarget): Promise<MediaAcquisitionFileSnapshot[]> {
  const platform = normalizeMediaAcquisitionPlatform(target.targetPlatform)
  const rootId = normalizeMediaAcquisitionRootFolder(platform, target.targetParentFileId)
  const queue = [{ id: rootId, path: '' }]
  const files: MediaAcquisitionFileSnapshot[] = []
  const visitedDirectories = new Set<string>()

  while (queue.length) {
    const current = queue.shift()!
    if (visitedDirectories.has(current.id)) continue
    visitedDirectories.add(current.id)
    const entries = await listDirectory(target, platform, current.id, current.path)
    for (const entry of entries) {
      if (entry.isDir) queue.push({ id: entry.id, path: entry.path })
      else files.push({ id: entry.id, name: entry.name, path: entry.path, size: entry.size, parentId: entry.parentId })
    }
  }
  return files
}

export async function listMediaAcquisitionDirectoryEntries(target: MediaAcquisitionTarget, parentId?: string, parentPath = ''): Promise<MediaAcquisitionDirectoryEntry[]> {
  const platform = normalizeMediaAcquisitionPlatform(target.targetPlatform)
  const rootId = normalizeMediaAcquisitionRootFolder(platform, target.targetParentFileId)
  return listDirectory(target, platform, parentId || rootId, parentPath)
}

export async function resolveMediaAcquisitionLeafFolder(target: MediaAcquisitionTarget): Promise<{ id: string; path: string } | null> {
  const platform = normalizeMediaAcquisitionPlatform(target.targetPlatform)
  let parentId = normalizeMediaAcquisitionRootFolder(platform, target.targetParentFileId)
  let parentPath = ''
  for (const segment of buildMediaAcquisitionLeafPath(target)) {
    const entry = (await listDirectory(target, platform, parentId, parentPath)).find(item => item.isDir && item.name === segment)
    if (!entry) return null
    parentId = entry.id
    parentPath = entry.path
  }
  return { id: parentId, path: parentPath || '/' }
}

/** Final side-effect gate: reread the real season directory immediately before a transfer. */
export interface MediaAcquisitionEpisodeCoverage {
  seasonNumber: number
  covered: boolean
  requestedEpisodes: number[]
  obtainedEpisodes: number[]
}

/** Read every requested season immediately before a provider write. */
export async function inspectMediaAcquisitionRequestedEpisodeCoverage(target: MediaAcquisitionTarget): Promise<{ covered: boolean; requestedEpisodes: number[]; obtainedEpisodes: number[]; seasons: MediaAcquisitionEpisodeCoverage[] }> {
  const seasonTargets = target.seasonTargets?.length
    ? target.seasonTargets
    : target.seasonNumber ? [{ seasonNumber: target.seasonNumber, missingEpisodes: target.missingEpisodes || [] }] : []
  if (!seasonTargets.length || !['tv', 'anime'].includes(target.mediaType)) return { covered: false, requestedEpisodes: [], obtainedEpisodes: [], seasons: [] }
  const seasons: MediaAcquisitionEpisodeCoverage[] = []
  for (const seasonTarget of seasonTargets) {
    const requestedEpisodes = [...new Set(seasonTarget.missingEpisodes || [])].filter(episode => Number.isInteger(episode) && episode > 0).sort((left, right) => left - right)
    if (!requestedEpisodes.length) continue
    const seasonTargetView = { ...target, seasonTargets: undefined, seasonNumber: seasonTarget.seasonNumber, missingEpisodes: requestedEpisodes }
    const leaf = await resolveMediaAcquisitionLeafFolder(seasonTargetView)
    const entries = leaf ? await listMediaAcquisitionDirectoryEntries(seasonTargetView, leaf.id, leaf.path) : []
    const obtainedEpisodes = extractObtainedEpisodeNumbers(entries.filter(entry => !entry.isDir && VIDEO_EXT.test(entry.name)).map(entry => entry.name), seasonTarget.seasonNumber)
    seasons.push({ seasonNumber: seasonTarget.seasonNumber, covered: requestedEpisodes.every(episode => obtainedEpisodes.includes(episode)), requestedEpisodes, obtainedEpisodes })
  }
  const requestedEpisodes = seasons.flatMap(season => season.requestedEpisodes)
  const obtainedEpisodes = seasons.flatMap(season => season.obtainedEpisodes)
  return { covered: seasons.length > 0 && seasons.every(season => season.covered), requestedEpisodes, obtainedEpisodes, seasons }
}

async function listDirectory(target: MediaAcquisitionTarget, platform: string, parentId: string, parentPath: string): Promise<MediaAcquisitionDirectoryEntry[]> {
  if (platform === 'aliyun') {
    const page = await AliTrash.ApiDirFileListNoLock(target.targetUserId, target.targetDriveId, parentId || 'root', '', 'name ASC', '', 0)
    return page.items.map(item => toEntry(item.file_id, item.name, item.isDir, Number(item.size || 0), parentPath, parentId))
  }
  if (platform === '115') {
    const entries: MediaAcquisitionDirectoryEntry[] = []
    let offset = 0
    while (true) {
      const page = await apiDrive115FileList(target.targetUserId, parentId.includes('root') ? '0' : parentId, 200, offset, true, { silent: true })
      entries.push(...page.map(item => toEntry(String(item.fid), item.fn, String(item.fc) === '0', Number(item.fs || 0), parentPath, parentId)))
      if (page.length < 200) break
      offset += page.length
    }
    return entries
  }
  if (platform === 'cloud123') {
    const entries: MediaAcquisitionDirectoryEntry[] = []
    let cursor: string | number = ''
    while (true) {
      const page = await apiCloud123FileListPage(target.targetUserId, parentId.includes('root') ? '0' : parentId, 100, false, '', 0, cursor)
      entries.push(...page.items.map(item => toEntry(String(item.fileId), item.filename, item.type === 1, Number(item.size || 0), parentPath, parentId)))
      if (page.items.length < 100 || page.lastFileId <= 0 || String(page.lastFileId) === String(cursor)) break
      cursor = page.lastFileId
    }
    return entries
  }
  if (platform === 'pikpak') {
    const entries: MediaAcquisitionDirectoryEntry[] = []
    let cursor = ''
    do {
      const page = await apiPikPakFileList(target.targetUserId, parentId, 100, cursor)
      entries.push(...page.items.map(item => toEntry(item.id, item.name, String(item.kind || '').includes('folder'), item.size === undefined ? undefined : Number(item.size), parentPath, parentId)))
      cursor = page.nextPageToken
    } while (cursor)
    return entries
  }
  if (platform === 'quark') {
    const entries: MediaAcquisitionDirectoryEntry[] = []
    let pageNo = 1
    while (true) {
      const page = await apiQuarkFileList(target.targetUserId, parentId.includes('root') ? 'quark_root' : parentId, 100, pageNo)
      entries.push(...page.items.map(item => toEntry(String(item.fid), item.file_name, Number(item.file_type || 0) === 0, optionalSize(item.size), parentPath, parentId)))
      if (page.items.length < 100 || entries.length >= page.total) break
      pageNo += 1
    }
    return entries
  }
  if (platform === 'guangya') {
    const items = await apiGuangyaFileList(target.targetUserId, parentId, Number.MAX_SAFE_INTEGER)
    return items.map(item => toEntry(getGuangyaFileId(item), getGuangyaFileName(item), isGuangyaDir(item), optionalSize(item.size ?? item.fileSize ?? item.file_size), parentPath, parentId))
  }
  throw new Error(`${target.targetPlatform} 暂不支持 Agent 入库核验`)
}

function toEntry(id: string, name: string, isDir: boolean, size: number | undefined, parentPath: string, parentId: string): MediaAcquisitionDirectoryEntry {
  return { id, name, isDir, size, parentId, path: `${parentPath}/${name}`.replace(/\/+/g, '/') }
}

function optionalSize(value: unknown): number | undefined {
  const size = Number(value)
  return Number.isFinite(size) && size >= 0 ? size : undefined
}
