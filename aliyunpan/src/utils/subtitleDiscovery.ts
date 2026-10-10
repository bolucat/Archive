export interface SubtitleDirectoryEntry {
  file_id?: string
  user_id?: string
  drive_id?: string
  name?: string
  html?: string
  ext?: string
  isDir?: boolean
}

export const subtitleFileKey = (file: SubtitleDirectoryEntry) => JSON.stringify([file.user_id || '', file.drive_id || '', file.file_id || file.name || file.html])

// List metadata only: signed download URLs are resolved when a subtitle is selected.
export async function discoverSubtitleFiles<T extends SubtitleDirectoryEntry>(
  directoryId: string,
  includeSubfolders: boolean,
  list: (id: string) => Promise<T[]>,
  isCurrent: () => boolean = () => true
): Promise<T[]> {
  const queue = [{ id: directoryId, path: '' }]
  const visited = new Set<string>()
  const files = new Map<string, T>()
  while (queue.length && isCurrent()) {
    const directory = queue.shift()!
    if (visited.has(directory.id)) continue
    visited.add(directory.id)
    const items = await list(directory.id)
    if (!isCurrent()) return []
    for (const item of items) {
      const name = item.name || item.html || ''
      if (item.isDir) {
        if (includeSubfolders && item.file_id) queue.push({ id: item.file_id, path: `${directory.path}${name}/` })
      } else if (/^(srt|vtt|ass|ssa)$/i.test(item.ext || name.split('.').pop() || '')) {
        const key = subtitleFileKey(item)
        if (!files.has(key)) files.set(key, { ...item, html: `${directory.path}${name}` })
      }
    }
  }
  return [...files.values()]
}
