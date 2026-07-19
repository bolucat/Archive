import path from 'path'

interface BtTaskLike {
  dir?: string
  totalLength?: string | number
  completedLength?: string | number
  files?: Array<{ path?: string; selected?: string | boolean; length?: string | number; completedLength?: string | number }>
  bittorrent?: { info?: { name?: string } }
}

interface DownloadPathInfo {
  DownSavePath: string
  name: string
  localFilePath?: string
}

function commonDirectory(filePaths: string[]): string {
  let common = path.dirname(filePaths[0])
  for (let i = 1; i < filePaths.length; i++) {
    const directory = path.dirname(filePaths[i])
    while (common && common !== path.dirname(common)) {
      const relative = path.relative(common, directory)
      if (relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative))) break
      common = path.dirname(common)
    }
  }
  return common
}

export function resolveBtDownloadTarget(task: BtTaskLike): { localFilePath: string; name: string; isDir: boolean } | null {
  const filePaths = (task.files || []).map((file) => String(file.path || '').trim()).filter(Boolean)
  if (!filePaths.length) return null

  if (filePaths.length === 1) {
    const localFilePath = path.normalize(filePaths[0])
    return { localFilePath, name: path.basename(localFilePath), isDir: false }
  }

  const localFilePath = commonDirectory(filePaths.map((filePath) => path.normalize(filePath))) || path.normalize(task.dir || '')
  const fallbackName = String(task.bittorrent?.info?.name || '').trim()
  return { localFilePath, name: path.basename(localFilePath) || path.basename(fallbackName), isDir: true }
}

export function isBtContentComplete(task: BtTaskLike): boolean {
  const totalLength = Number(task.totalLength || 0)
  const completedLength = Number(task.completedLength || 0)
  if (totalLength <= 0 || completedLength < totalLength) return false

  const selectedFiles = (task.files || []).filter((file) => file.selected !== false && file.selected !== 'false')
  if (!selectedFiles.length) return false
  return selectedFiles.every((file) => {
    const length = Number(file.length || 0)
    const fileCompletedLength = Number(file.completedLength || 0)
    return fileCompletedLength >= length
  })
}

export function resolveDownloadOpenPath(info: DownloadPathInfo): string {
  return info.localFilePath ? path.normalize(info.localFilePath) : path.join(info.DownSavePath, info.name)
}

export function resolveFollowedBtGid(task: { followedBy?: unknown }): string {
  if (!Array.isArray(task.followedBy)) return ''
  return String(task.followedBy.find((gid) => typeof gid === 'string' && gid.trim()) || '').trim()
}

export function resolveLegacyMagnetPath(savePath: string, source: string): string {
  if (!/^magnet:\?/i.test(source.trim())) return ''
  try {
    const name = new URL(source).searchParams.get('dn')?.trim() || ''
    return name ? path.join(savePath, path.basename(name)) : ''
  } catch {
    return ''
  }
}

export function buildBtControlFileCandidates(info: DownloadPathInfo, source: string): string[] {
  const targets = [resolveDownloadOpenPath(info), resolveLegacyMagnetPath(info.DownSavePath, source)].filter(Boolean)
  return [...new Set(targets.map((target) => `${target}.aria2`))]
}
