import type { IAliGetFileModel } from '../../aliapi/alimodels'
import { listDriveToolChildren } from './directLinks'

export type DuplicateScanMode = 'helperName' | 'contentHash'

export interface DuplicateDriveTarget {
  userId: string
  driveId: string
  rootId: string
  name: string
}

export interface DuplicateFileItem {
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
  contentHash: string
}

export interface DuplicateGroup {
  key: string
  label: string
  files: DuplicateFileItem[]
}

export interface DuplicateScanResult {
  groups: DuplicateGroup[]
  scannedDirs: number
  scannedFiles: number
  truncated: boolean
  report: string
}

export interface DuplicateDeleteResult {
  total: number
  success: number
  failed: number
  deletedFileIds: string[]
  deletedFileKeys: string[]
  report: string
}

const normalizeDuplicateName = (name: string) => String(name || '')
  .replace(/[０-９]/g, char => String.fromCharCode(char.charCodeAt(0) - 65248))
  .replace(/[（]/g, '(')
  .replace(/[）]/g, ')')
  .replace(/\u00a0/g, ' ')
  .replace(/[\u200b-\u200d\ufeff]/g, '')
  .replace(/\s+/g, ' ')
  .trim()

const parseHelperDuplicateName = (name: string, numbers: Set<string>) => {
  const normalized = normalizeDuplicateName(name)
  const match = normalized.match(/^(.*?)[(](\d+)[)]\s*$/u) || normalized.match(/^(.*?)[(](\d+)[)](\.[a-z0-9]{1,12})$/iu)
  if (!match || !numbers.has(match[2])) return null
  return {
    baseName: match[1].trim(),
    extension: match[3] || ''
  }
}

const toDuplicateFile = (item: IAliGetFileModel, userId: string, path: string): DuplicateFileItem => ({
  userId,
  driveId: item.drive_id,
  name: item.name,
  fileId: item.file_id,
  parentFileId: item.parent_file_id,
  size: item.size || 0,
  sizeStr: item.sizeStr || '',
  time: item.time || 0,
  timeStr: item.timeStr || '',
  icon: item.icon || 'iconwenjian',
  path,
  contentHash: (item as any).content_hash || ''
})

export const scanDriveDuplicates = async (
  targets: DuplicateDriveTarget[],
  mode: DuplicateScanMode = 'helperName',
  options: { numbers?: string; maxDirs?: number; maxFiles?: number } = {}
): Promise<DuplicateScanResult> => {
  const numbers = new Set((options.numbers || '1,2,3').split(/[\s,，、]+/).filter(Boolean).map(item => normalizeDuplicateName(item)))
  const maxDirs = options.maxDirs || 2000
  const maxFiles = options.maxFiles || 10000
  const candidates = new Map<string, { label: string; files: DuplicateFileItem[] }>()
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
        let key = ''
        let label = ''
        if (mode === 'helperName') {
          const parsed = parseHelperDuplicateName(item.name, numbers)
          if (!parsed) continue
          key = `${parsed.baseName.toLocaleLowerCase()}${parsed.extension.toLocaleLowerCase()}`
          label = `${parsed.baseName}${parsed.extension}`
        } else if ((item as any).content_hash) {
          key = `${item.size || 0}:${(item as any).content_hash.toLocaleLowerCase()}`
          label = `${item.sizeStr || item.size} - ${(item as any).content_hash}`
        }
        if (!key) continue
        const entry = candidates.get(key) || { label, files: [] }
        entry.files.push(toDuplicateFile(item, target.userId, itemPath))
        candidates.set(key, entry)
      }
    }
  }

  const groups = Array.from(candidates.entries())
    .filter(([, entry]) => mode === 'helperName' ? entry.files.length > 0 : entry.files.length > 1)
    .map(([key, entry]) => ({ key, label: mode === 'helperName' ? entry.label : `${entry.files[0].sizeStr || entry.files[0].size} - ${entry.files[0].contentHash}`, files: entry.files }))
    .sort((a, b) => b.files.reduce((sum, item) => sum + item.size, 0) - a.files.reduce((sum, item) => sum + item.size, 0))

  return {
    groups,
    scannedDirs,
    scannedFiles,
    truncated,
    report: `重复项扫描完成：找到 ${groups.length} 组，${groups.reduce((sum, group) => sum + group.files.length, 0)} 个候选文件；已扫 ${scannedDirs} 个目录、${scannedFiles} 个文件${truncated ? '，结果可能未扫全' : ''}`
  }
}

export const deleteDriveDuplicates = async (files: DuplicateFileItem[]): Promise<DuplicateDeleteResult> => {
  const { default: AliFileCmd } = await import('../../aliapi/filecmd')
  const groups = new Map<string, DuplicateFileItem[]>()
  for (const file of files.filter(item => item.userId && item.driveId && item.fileId)) {
    const key = `${file.userId}\n${file.driveId}`
    groups.set(key, [...(groups.get(key) || []), file])
  }
  let success = 0
  const deletedFileIds: string[] = []
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
    deletedFileIds.push(...deleted)
    deletedFileKeys.push(...deleted.map(fileId => `${userId}\n${driveId}\n${fileId}`))
  }
  const total = Array.from(groups.values()).reduce((sum, group) => sum + group.length, 0)
  return { total, success, failed: total - success, deletedFileIds, deletedFileKeys, report: `重复文件删除完成：成功 ${success}/${total}${total > success ? `，失败 ${total - success}` : ''}` }
}
