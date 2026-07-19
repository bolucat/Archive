const ASSRT_API_BASE = 'https://api.assrt.net/v1'

export interface AssrtSubtitleCandidate {
  id: number
  name: string
  videoName: string
  language: string
  subtitleType: string
  score: number
  downloads: number
}

export interface AssrtSubtitleFile {
  name: string
  size?: string
  url: string
}

function normalizeToken(token: unknown): string {
  const normalized = String(token || '').trim()
  if (!/^[A-Za-z0-9]{32}$/.test(normalized)) throw new Error('ASSRT Token 格式无效')
  return normalized
}

async function requestAssrt(path: string, token: unknown, params: Record<string, string> = {}) {
  const url = new URL(`${ASSRT_API_BASE}${path}`)
  for (const [key, value] of Object.entries(params)) url.searchParams.set(key, value)
  const response = await fetch(url, { headers: { Authorization: `Bearer ${normalizeToken(token)}`, 'User-Agent': 'BoxPlayer/5' } })
  const data = await response.json().catch(() => ({})) as any
  if (!response.ok || Number(data?.status) !== 0) throw new Error(data?.errmsg || data?.message || `ASSRT 请求失败 (${response.status})`)
  return data
}

export async function searchAssrtSubtitles(token: unknown, query: unknown): Promise<AssrtSubtitleCandidate[]> {
  const keyword = String(query || '').trim()
  if (keyword.length < 3 || keyword.length > 200) throw new Error('字幕搜索关键词长度应为 3 到 200 个字符')
  const data = await requestAssrt('/sub/search', token, { q: keyword, cnt: '15', no_muxer: '1' })
  return (data?.sub?.subs || []).map((item: any) => ({
    id: Number(item.id), name: String(item.native_name || item.title || ''), videoName: String(item.videoname || ''), language: String(item?.lang?.desc || ''), subtitleType: String(item.subtype || ''), score: Number(item.vote_score || 0), downloads: Number(item.down_count || 0)
  })).filter((item: AssrtSubtitleCandidate) => Number.isFinite(item.id) && item.id > 0)
}

export async function getAssrtSubtitleFiles(token: unknown, subtitleId: unknown): Promise<AssrtSubtitleFile[]> {
  const id = Number(subtitleId)
  if (!Number.isInteger(id) || id <= 0) throw new Error('字幕 ID 无效')
  const data = await requestAssrt('/sub/detail', token, { id: String(id) })
  const subtitle = data?.sub?.subs?.[0]
  const files = Array.isArray(subtitle?.filelist) ? subtitle.filelist : []
  return files.map((file: any) => ({ name: String(file.f || ''), size: typeof file.s === 'string' ? file.s : undefined, url: String(file.url || '') })).filter((file: AssrtSubtitleFile) => file.name && /^https?:\/\//i.test(file.url))
}
