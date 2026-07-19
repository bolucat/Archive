import Config from '../../config'
import useSettingStore from '../../setting/settingstore'

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

function apiUrl(path: string, params: Record<string, string>) {
  const url = new URL(`${Config.BOXPLAYER_API_URL.replace(/\/+$/, '')}/api/subtitles/assrt/${path}`)
  for (const [key, value] of Object.entries(params)) url.searchParams.set(key, value)
  return url
}

async function parseResponse<T>(response: Response): Promise<T> {
  const body = await response.json().catch(() => ({})) as any
  if (!response.ok) throw new Error(body?.statusMessage || body?.message || `字幕服务请求失败 (${response.status})`)
  return body.data as T
}

export async function searchAssrtSubtitleCandidates(query: string): Promise<AssrtSubtitleCandidate[]> {
  const token = useSettingStore().mediaAcquisitionAssrtToken.trim()
  if (token) return window.Electron?.ipcRenderer?.invoke('mediaAcquisition:assrtSearch', { token, query }) || []
  return parseResponse<AssrtSubtitleCandidate[]>(await fetch(apiUrl('search', { query })))
}

export async function getAssrtSubtitleCandidateFiles(subtitleId: number): Promise<AssrtSubtitleFile[]> {
  const token = useSettingStore().mediaAcquisitionAssrtToken.trim()
  if (token) return window.Electron?.ipcRenderer?.invoke('mediaAcquisition:assrtDetail', { token, subtitleId }) || []
  return parseResponse<AssrtSubtitleFile[]>(await fetch(apiUrl('detail', { id: String(subtitleId) })))
}
