import { SUBTITLE_DOWNLOAD_API_KEY, SUBTITLE_SEARCH_API_KEY } from '../secrets.generated'

export interface SubtitleSearchResult {
  id: string
  name: string
  language: string
  format: SubtitleSearchFormat
  downloadCount: number
  fileId: number
}

export type SubtitleSearchFormat = 'srt' | 'ass' | 'vtt' | 'unknown'

export interface SubtitleDownloadResult {
  fileName: string
  link: string
}

type FetchLike = typeof fetch

const SEARCH_API_KEY = SUBTITLE_SEARCH_API_KEY
const DOWNLOAD_API_KEY = SUBTITLE_DOWNLOAD_API_KEY
const OPEN_SUBTITLES_API = 'https://api.opensubtitles.com/api/v1'

async function readJson<T>(response: Response): Promise<T> {
  if (!response.ok) throw new Error(`HTTP ${response.status}`)
  return response.json() as Promise<T>
}

export function buildSubtitleSearchUrl(query: string, language: string) {
  const url = new URL(`${OPEN_SUBTITLES_API}/subtitles`)
  const trimmedQuery = query.trim()
  if (/^\d+$/.test(trimmedQuery)) {
    url.searchParams.set('tmdb_id', trimmedQuery)
  } else {
    url.searchParams.set('query', trimmedQuery)
  }
  url.searchParams.set('languages', language)
  return url.toString()
}

export async function searchSubtitles(query: string, language: string, fetcher: FetchLike = fetch): Promise<SubtitleSearchResult[]> {
  if (!query.trim()) return []
  const response = await fetcher(buildSubtitleSearchUrl(query, language), {
    headers: {
      'Api-Key': SEARCH_API_KEY
    }
  })
  const data = await readJson<any>(response as Response)
  return (data.data || [])
    .flatMap((item: any): SubtitleSearchResult[] => {
      const attributes = item?.attributes || {}
      const files = Array.isArray(attributes.files) ? attributes.files : []
      return files.flatMap((file: any): SubtitleSearchResult[] => {
        const fileId = Number(file?.file_id)
        const fileName = String(file?.file_name || '').trim()
        const name = String(attributes.release || fileName).trim()
        if (!fileId || !name) return []
        return [{
          id: `${item.id || 'subtitle'}:${fileId}`,
          name,
          language: String(attributes.language || language),
          format: getSubtitleSearchFormat(fileName),
          downloadCount: Number(attributes.new_download_count || 0),
          fileId
        }]
      })
    })
    .sort((a: SubtitleSearchResult, b: SubtitleSearchResult) => b.downloadCount - a.downloadCount) as SubtitleSearchResult[]
}

export async function getSubtitleDownload(fileId: number, fetcher: FetchLike = fetch): Promise<SubtitleDownloadResult> {
  const response = await fetcher(`${OPEN_SUBTITLES_API}/download`, {
    method: 'POST',
    headers: {
      'Api-Key': DOWNLOAD_API_KEY,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      file_id: fileId
    })
  })
  const data = await readJson<any>(response as Response)
  if (!data.link || !data.file_name) throw new Error('Invalid subtitle download response')
  return {
    fileName: String(data.file_name),
    link: String(data.link)
  }
}

export function getSubtitleSearchFormat(fileName: string): SubtitleSearchFormat {
  const ext = fileName.split('?')[0].split('#')[0].split('.').pop()?.toLowerCase() || 'srt'
  if (ext === 'ass' || ext === 'ssa') return 'ass'
  if (ext === 'vtt') return 'vtt'
  if (ext === 'srt') return 'srt'
  return 'unknown'
}

export function getSubtitleExtension(fileName: string) {
  const format = getSubtitleSearchFormat(fileName)
  return format === 'unknown' ? 'srt' : format
}

export function decodeSubtitleBuffer(buffer: ArrayBuffer) {
  const bytes = new Uint8Array(buffer)
  if (bytes[0] === 0xff && bytes[1] === 0xfe) return new TextDecoder('utf-16le').decode(buffer)
  if (bytes[0] === 0xfe && bytes[1] === 0xff) return new TextDecoder('utf-16be').decode(buffer)

  const sampleLength = Math.min(bytes.length, 256)
  let evenNulls = 0
  let oddNulls = 0
  for (let index = 0; index < sampleLength; index += 1) {
    if (bytes[index] !== 0) continue
    if (index % 2 === 0) evenNulls += 1
    else oddNulls += 1
  }
  if (oddNulls > sampleLength / 8) return new TextDecoder('utf-16le').decode(buffer)
  if (evenNulls > sampleLength / 8) return new TextDecoder('utf-16be').decode(buffer)
  return new TextDecoder('utf-8').decode(buffer)
}

export function formatSubtitleDownloadCount(count: number) {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`
  if (count >= 1000) return `${(count / 1000).toFixed(1)}K`
  return String(count)
}
