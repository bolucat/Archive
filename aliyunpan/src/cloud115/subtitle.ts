export interface Drive115SubtitleSource {
  url: string
  language: string
  headers?: Record<string, string>
}

export function mapDrive115SubtitleItems(items: unknown): Drive115SubtitleSource[] {
  if (!Array.isArray(items)) return []
  return items.flatMap((item: any) => {
    const url = typeof item?.url === 'string' ? item.url : typeof item?.url?.url === 'string' ? item.url.url : ''
    if (!url) return []
    return [{ url, language: String(item.language || item.lang || item.title || item.name || '字幕') }]
  })
}
