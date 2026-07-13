import type { IAliGetFileModel } from '../../aliapi/alimodels'
import { listDriveToolChildren } from './directLinks'

export interface EmptyDirItem {
  name: string
  fileId: string
  parentFileId: string
  driveId: string
  userId: string
  path: string
}

export interface EmptyDirScanResult {
  rootId: string
  scannedDirs: number
  emptyDirs: EmptyDirItem[]
  truncated: boolean
  report: string
}

export interface EmptyDirDeleteResult {
  total: number
  success: number
  failed: number
  deletedFileKeys: string[]
  report: string
}

const toEmptyDir = (item: IAliGetFileModel, userId: string, path: string): EmptyDirItem => ({
  name: item.name,
  fileId: item.file_id,
  parentFileId: item.parent_file_id,
  driveId: item.drive_id,
  userId,
  path
})

export const scanDriveEmptyDirs = async (userId: string, driveId: string, rootId: string, maxDirs = 500): Promise<EmptyDirScanResult> => {
  const queue: { fileId: string; path: string; item?: IAliGetFileModel }[] = [{ fileId: rootId, path: '' }]
  const emptyDirs: EmptyDirItem[] = []
  let scannedDirs = 0
  let truncated = false

  while (queue.length) {
    if (scannedDirs >= maxDirs) {
      truncated = true
      break
    }
    const current = queue.shift()!
    scannedDirs += 1
    const children = await listDriveToolChildren(userId, driveId, current.fileId).catch(() => [])
    const childDirs = children.filter(item => item.isDir)
    if (current.item && children.length === 0) {
      emptyDirs.push(toEmptyDir(current.item, userId, current.path || current.item.name))
    }
    for (const dir of childDirs) {
      queue.push({ fileId: dir.file_id, path: current.path ? `${current.path}/${dir.name}` : dir.name, item: dir })
    }
  }

  const report = `空目录扫描完成：找到 ${emptyDirs.length} 个，已扫 ${scannedDirs} 个目录${truncated ? '，结果可能未扫全' : ''}`
  return { rootId, scannedDirs, emptyDirs, truncated, report }
}

export const deleteDriveEmptyDirs = async (dirs: EmptyDirItem[]): Promise<EmptyDirDeleteResult> => {
  const validDirs = dirs.filter(item => item.userId && item.driveId && item.fileId)
  if (!validDirs.length) return { total: 0, success: 0, failed: 0, deletedFileKeys: [], report: '没有可删除的空目录' }
  const { default: AliFileCmd } = await import('../../aliapi/filecmd')
  const groups = new Map<string, EmptyDirItem[]>()
  for (const item of validDirs) {
    const key = `${item.userId}\n${item.driveId}`
    groups.set(key, [...(groups.get(key) || []), item])
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
  const failed = validDirs.length - success
  return {
    total: validDirs.length,
    success,
    failed,
    deletedFileKeys,
    report: `空目录删除完成：成功 ${success}/${validDirs.length}${failed ? `，失败 ${failed}` : ''}`
  }
}
