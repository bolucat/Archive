import type { IAliGetFileModel } from '../../aliapi/alimodels'
import { humanSize } from '../format'
import { listDriveToolChildren } from './directLinks'
import type { DuplicateDriveTarget } from './duplicates'

export type LargeFileScanMode = 'size' | 'video' | 'doc' | 'zip' | 'others' | 'size5000' | 'size1000' | 'size100'

export interface LargeFileItem {
  userId: string
  driveId: string
  name: string
  fileId: string
  parentFileId: string
  size: number
  sizeStr: string
  time: number
  timeStr: string
  icon: string
  path: string
  category: string
  ext: string
}

export interface LargeFileScanResult {
  files: LargeFileItem[]
  scannedDirs: number
  scannedFiles: number
  truncated: boolean
  report: string
}

export interface LargeFileDeleteResult {
  total: number
  success: number
  failed: number
  deletedFileKeys: string[]
  report: string
}

const mb = 1024 * 1024
const gb = 1024 * mb
const docExts = new Set(['.doc', '.docx', '.xls', '.xlsx', '.ppt', '.pptx', '.pdf', '.txt', '.md', '.epub'])
const zipExts = new Set(['.zip', '.rar', '.7z', '.tar', '.gz', '.bz2', '.xz', '.iso'])
const normalizeExt = (value: string) => {
  const ext = String(value || '').trim().toLocaleLowerCase()
  if (!ext) return ''
  return ext.startsWith('.') ? ext : `.${ext}`
}

const thresholdForMode = (mode: LargeFileScanMode, customSizeMB: number) => {
  if (mode === 'size5000') return 5 * gb
  if (mode === 'size1000' || mode === 'video' || mode === 'doc' || mode === 'zip' || mode === 'others') return gb
  if (mode === 'size100') return 100 * mb
  return Math.max(1, customSizeMB) * mb
}

const matchesMode = (item: IAliGetFileModel, mode: LargeFileScanMode, minSize: number) => {
  if (item.isDir || (item.size || 0) < minSize) return false
  const ext = normalizeExt(item.ext || item.mime_extension || item.name?.split('.').pop() || '')
  if (mode.startsWith('size')) return true
  if (mode === 'video') return item.category === 'video'
  if (mode === 'doc') return item.category === 'doc' || docExts.has(ext)
  if (mode === 'zip') return zipExts.has(ext)
  if (mode === 'others') return item.category !== 'video' && item.category !== 'doc' && !docExts.has(ext) && !zipExts.has(ext)
  return true
}

const toLargeFile = (item: IAliGetFileModel, userId: string, path: string): LargeFileItem => ({
  userId,
  driveId: item.drive_id,
  name: item.name,
  fileId: item.file_id,
  parentFileId: item.parent_file_id,
  size: item.size || 0,
  sizeStr: item.sizeStr || humanSize(item.size || 0),
  time: item.time || 0,
  timeStr: item.timeStr || '',
  icon: item.icon || 'iconwenjian',
  path,
  category: item.category || '',
  ext: item.ext || ''
})

export const scanDriveLargeFiles = async (
  targets: DuplicateDriveTarget[],
  mode: LargeFileScanMode,
  options: { customSizeMB?: number; maxDirs?: number; maxFiles?: number } = {}
): Promise<LargeFileScanResult> => {
  const minSize = thresholdForMode(mode, options.customSizeMB || 100)
  const maxDirs = options.maxDirs || 2000
  const maxFiles = options.maxFiles || 20000
  const files: LargeFileItem[] = []
  let scannedDirs = 0
  let scannedFiles = 0
  let truncated = false

  for (const target of targets) {
    let targetDirs = 0
    let targetFiles = 0
    const queue: { fileId: string; path: string }[] = [{ fileId: target.rootId, path: target.name }]
    while (queue.length) {
      if (targetDirs >= maxDirs || targetFiles >= maxFiles) {
        truncated = true
        break
      }
      const current = queue.shift()!
      scannedDirs += 1
      targetDirs += 1
      const children = await listDriveToolChildren(target.userId, target.driveId, current.fileId).catch(() => [])
      for (const item of children) {
        const itemPath = current.path ? `${current.path}/${item.name}` : item.name
        if (item.isDir) {
          queue.push({ fileId: item.file_id, path: itemPath })
          continue
        }
        scannedFiles += 1
        targetFiles += 1
        if (matchesMode(item, mode, minSize)) files.push(toLargeFile(item, target.userId, itemPath))
      }
    }
  }

  files.sort((a, b) => b.size - a.size)
  return {
    files,
    scannedDirs,
    scannedFiles,
    truncated,
    report: `大文件扫描完成：找到 ${files.length} 个，已扫 ${scannedDirs} 个目录、${scannedFiles} 个文件${truncated ? '，结果可能未扫全' : ''}`
  }
}

export const deleteDriveLargeFiles = async (files: LargeFileItem[]): Promise<LargeFileDeleteResult> => {
  const { default: AliFileCmd } = await import('../../aliapi/filecmd')
  const groups = new Map<string, LargeFileItem[]>()
  for (const file of files.filter(item => item.userId && item.driveId && item.fileId)) {
    const key = `${file.userId}\n${file.driveId}`
    groups.set(key, [...(groups.get(key) || []), file])
  }
  let success = 0
  const deletedFileKeys: string[] = []
  for (const [key, group] of groups) {
    const [userId, driveId] = key.split('\n')
    const deleted = driveId.startsWith('webdav:')
      ? await (async () => {
        const { deleteWebDavPath, getWebDavConnection, getWebDavConnectionId } = await import('../webdavClient')
        const connection = getWebDavConnection(getWebDavConnectionId(driveId))
        if (!connection) return []
        const ids: string[] = []
        for (const item of group) {
          try {
            await deleteWebDavPath(connection, item.fileId)
            ids.push(item.fileId)
          } catch {}
        }
        return ids
      })()
      : await AliFileCmd.ApiTrashBatch(userId, driveId, group.map(item => item.fileId))
    success += deleted.length
    deletedFileKeys.push(...deleted.map(fileId => `${userId}\n${driveId}\n${fileId}`))
  }
  const total = Array.from(groups.values()).reduce((sum, group) => sum + group.length, 0)
  return { total, success, failed: total - success, deletedFileKeys, report: `大文件删除完成：成功 ${success}/${total}${total > success ? `，失败 ${total - success}` : ''}` }
}
