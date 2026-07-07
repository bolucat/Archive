/**
 * Multi-source music SDK aggregator.
 * Provides searchMusic and findMusic across NetEase (wy) and Kugou (kg).
 */
import wy, { type OldMusicInfo, type SearchResult } from './wy/index'
import kg from './kg/index'

const sources = ['wy', 'kg'] as const

export type MusicSource = typeof sources[number]

export interface FindMusicParams {
  name: string
  singer?: string
  interval?: string
  albumName?: string
}

export interface FindMusicResult {
  musicInfo: OldMusicInfo
  source: MusicSource
}

export interface LyricResult {
  lyric: string
  tlyric?: string
  rlyric?: string
  lxlyric?: string
}

export interface LyricFetchDebugStep {
  source: MusicSource | 'search'
  status: 'ok' | 'empty' | 'error' | 'skipped'
  message: string
  lyricLength?: number
  tlyricLength?: number
  songName?: string
  singer?: string
  songmid?: string | number
  hash?: string
}

export interface LyricFetchDebug {
  query: FindMusicParams
  matched?: {
    source: MusicSource
    name: string
    singer: string
    songmid?: string | number
    hash?: string
  }
  steps: LyricFetchDebugStep[]
  finalStatus: 'ok' | 'no-match' | 'empty' | 'error'
}

export interface LyricDetailedResult {
  lyric: LyricResult | null
  debug: LyricFetchDebug
}

// ---- Search across all sources ----

export async function searchMusic(
  params: FindMusicParams,
  limit = 3
): Promise<SearchResult[]> {
  const tasks = sources.map((source) => {
    const sdk = source === 'wy' ? wy : kg
    const req: any = sdk.search(params.name, 1, limit)
    return req.promise.then((r: any) => r).catch(() => null)
  })
  const results = await Promise.all(tasks)
  return results.filter((r): r is SearchResult => r !== null)
}

// ---- Find matching music ----

function cleanStr(s: string): string {
  return s
    .toLowerCase()
    .replace(/[，、,]/g, '')
    .replace(/[（(].*?[)）]/g, '')
    .replace(/feat\.|ft\.|feat/g, '')
    .replace(/[^a-z0-9\u4e00-\u9fff]/g, '')
    .trim()
}

function sortSingers(singer: string): string {
  return singer
    .split('、')
    .map((s) => s.trim())
    .sort()
    .join('')
}

export async function findMusic(params: FindMusicParams): Promise<OldMusicInfo | null> {
  const searchParams: FindMusicParams = {
    name: cleanStr(params.name),
    singer: params.singer || '',
    albumName: params.albumName || '',
  }

  const allResults = await searchMusic({ name: params.name }, 10)
  const candidates: OldMusicInfo[] = []

  for (const result of allResults) {
    for (const item of result.list) {
      if (item.name) candidates.push(item)
    }
  }

  if (candidates.length === 0) return null

  const cleanName = cleanStr(params.name)
  const cleanSinger = sortSingers(searchParams.singer || '')

  let best: OldMusicInfo | null = null
  let bestScore = -1

  for (const item of candidates) {
    let score = 0
    const itemName = cleanStr(item.name)
    const itemSinger = sortSingers(item.singer || '')

    if (itemName === cleanName) score += 50
    else if (itemName.includes(cleanName) || cleanName.includes(itemName)) score += 30

    if (itemSinger === cleanSinger) score += 40
    else if (itemSinger.includes(cleanSinger) || cleanSinger.includes(itemSinger)) score += 15

    if (score > bestScore) {
      bestScore = score
      best = item
    }
  }

  return bestScore >= 30 ? best : null
}

// ---- Get lyric ----

export async function fetchLyric(
  params: FindMusicParams & { songmid?: any; hash?: string; interval?: string; _interval?: number }
): Promise<LyricResult | null> {
  const result = await fetchLyricDetailed(params)
  return result.lyric
}

export async function fetchLyricDetailed(
  params: FindMusicParams & { songmid?: any; hash?: string; interval?: string; _interval?: number }
): Promise<LyricDetailedResult> {
  const debug: LyricFetchDebug = {
    query: { name: params.name, singer: params.singer || '', interval: params.interval || '', albumName: params.albumName || '' },
    steps: [],
    finalStatus: 'empty'
  }

  let songInfo: any = null
  try {
    songInfo = await findMusic(params)
  } catch (e: any) {
    debug.steps.push({ source: 'search', status: 'error', message: e?.message || String(e) })
    debug.finalStatus = 'error'
    return { lyric: null, debug }
  }

  if (!songInfo) {
    debug.steps.push({ source: 'search', status: 'empty', message: '未搜索到匹配歌曲' })
    debug.finalStatus = 'no-match'
    return { lyric: null, debug }
  }

  const source = songInfo.source as MusicSource
  debug.matched = {
    source,
    name: songInfo.name || '',
    singer: songInfo.singer || '',
    songmid: songInfo.songmid,
    hash: songInfo.hash
  }

  const readFromSource = async (target: MusicSource): Promise<LyricResult | null> => {
    try {
      let lyric: LyricResult | null = null
      if (target === 'wy') {
        if (!songInfo.songmid) {
          debug.steps.push({ source: target, status: 'skipped', message: '缺少网易云 songmid' })
          return null
        }
        const req = wy.getLyric(songInfo.songmid)
        lyric = await req.promise
      } else {
        if (!songInfo.hash) {
          debug.steps.push({ source: target, status: 'skipped', message: '缺少酷狗 hash' })
          return null
        }
        const req = kg.getLyric({ name: songInfo.name, hash: songInfo.hash, interval: songInfo.interval, _interval: songInfo._interval })
        lyric = await req.promise
      }
      const lyricLength = lyric?.lyric?.length || 0
      debug.steps.push({
        source: target,
        status: lyricLength ? 'ok' : 'empty',
        message: lyricLength ? '歌词接口返回歌词' : '歌词接口返回空歌词',
        lyricLength,
        tlyricLength: lyric?.tlyric?.length || 0,
        songName: songInfo.name,
        singer: songInfo.singer,
        songmid: songInfo.songmid,
        hash: songInfo.hash
      })
      return lyricLength ? lyric : null
    } catch (e: any) {
      debug.steps.push({
        source: target,
        status: 'error',
        message: e?.message || String(e),
        songName: songInfo.name,
        singer: songInfo.singer,
        songmid: songInfo.songmid,
        hash: songInfo.hash
      })
      return null
    }
  }

  const primary = await readFromSource(source)
  if (primary) {
    debug.finalStatus = 'ok'
    return { lyric: primary, debug }
  }

  const fallbackSource = source === 'wy' ? 'kg' : 'wy'
  const fallback = await readFromSource(fallbackSource)
  if (fallback) {
    debug.finalStatus = 'ok'
    return { lyric: fallback, debug }
  }

  debug.finalStatus = debug.steps.some((step) => step.status === 'error') ? 'error' : 'empty'
  return { lyric: null, debug }
}

// ---- Get cover ----

export async function fetchCover(
  params: FindMusicParams
): Promise<string | null> {
  const songInfo = await findMusic(params)
  if (!songInfo) return null

  try {
    if (songInfo.source === 'wy') {
      const req = wy.getPic(songInfo.songmid)
      return await req.promise
    }
  } catch {}

  return songInfo.img || null
}
