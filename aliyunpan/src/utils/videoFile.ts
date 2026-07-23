const videoExtensions = new Set(['3gp', 'avi', 'flv', 'm2ts', 'm4v', 'mkv', 'mov', 'mp4', 'mpeg', 'mpg', 'rmvb', 'ts', 'vob', 'webm', 'wmv'])

export function isVideoFile(file: { name?: string, file_name?: string, html?: string, ext?: string, category?: string, isDir?: boolean } | undefined | null): boolean {
  if (!file || file.isDir) return false
  if (String(file.category || '').startsWith('video')) return true
  const name = String(file.name || file.file_name || file.html || '')
  const extension = String(file.ext || (name.includes('.') ? name.slice(name.lastIndexOf('.') + 1) : '')).replace(/^\./, '').toLowerCase()
  return videoExtensions.has(extension)
}
