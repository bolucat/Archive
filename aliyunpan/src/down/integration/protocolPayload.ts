export type ExternalDownloadSourceType = 'url' | 'magnet' | 'torrent-url' | 'torrent'

export interface ExternalDownloadPayload {
  source: string
  sourceType: ExternalDownloadSourceType
  filePath?: string
}

const isTorrentUrl = (source: string): boolean =>
  /^https?:\/\/.+\.torrent(?:[?#].*)?$/i.test(source)

export const parseExternalDownloadPayload = (value: string): ExternalDownloadPayload | null => {
  const source = value.trim()
  if (!source) return null
  if (/^magnet:\?/i.test(source)) return { source, sourceType: 'magnet' }
  if (isTorrentUrl(source)) return { source, sourceType: 'torrent-url' }
  if (/^https?:\/\//i.test(source)) return { source, sourceType: 'url' }
  if (/\.torrent$/i.test(source)) return { source, sourceType: 'torrent', filePath: source }
  return null
}
