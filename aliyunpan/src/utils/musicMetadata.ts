import axios from 'axios'
import DebugLog from './debuglog'

export interface LyricLine {
  time: number // seconds
  text: string
}

export interface MusicMetadata {
  title: string
  artist: string
  album: string
  cover: string // url
  durationMs?: number
  lrc: string // raw LRC text
  lines: LyricLine[]
  fromCache?: boolean
}

interface MetaQuery {
  filename: string
  durationSec?: number
  artistHint?: string
  titleHint?: string
  albumHint?: string
}

const META_TTL_MS = 24 * 60 * 60 * 1000
const memCache = new Map<string, { ts: number, data: MusicMetadata }>()
const inflight = new Map<string, Promise<MusicMetadata>>()

const COMMON_BRACKETS_RE = /[\(\[（【][^\)\]）】]*[\)\]）】]/g
const TRAILING_TAGS_RE = /\s*-\s*(official|mv|hd|hq|lossless|live|remix|cover|伴奏|纯音乐|高清|无损|完整版|live现场)\s*$/gi

function stripExt(name: string): string {
  if (!name) return ''
  const i = name.lastIndexOf('.')
  return i > 0 ? name.slice(0, i) : name
}

function cleanFragment(s: string): string {
  return s
    .replace(COMMON_BRACKETS_RE, ' ')
    .replace(TRAILING_TAGS_RE, ' ')
    .replace(/[_\.]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
}

export function guessArtistTitle(filename: string): { artist: string, title: string } {
  const base = cleanFragment(stripExt(filename))
  if (!base) return { artist: '', title: '' }
  const seps = [' - ', '_-_', ' – ', '–', ' — ', '—']
  for (const sep of seps) {
    const i = base.indexOf(sep)
    if (i > 0) {
      const a = base.slice(0, i).trim()
      const t = base.slice(i + sep.length).trim()
      if (a && t) return { artist: a, title: t }
    }
  }
  return { artist: '', title: base }
}

function buildKey(q: MetaQuery): string {
  const guess = guessArtistTitle(q.filename)
  const artist = (q.artistHint || guess.artist || '').trim()
  const title = (q.titleHint || guess.title || '').trim()
  const dur = q.durationSec ? Math.round(q.durationSec) : 0
  return `${artist}|${title}|${dur}`.toLowerCase()
}

export function parseLrc(lrc: string): LyricLine[] {
  if (!lrc) return []
  const out: LyricLine[] = []
  const lines = lrc.split(/\r?\n/)
  const tagRe = /\[(\d{1,2}):(\d{1,2}(?:[\.:]\d{1,3})?)\]/g
  for (const raw of lines) {
    if (!raw) continue
    const stamps: number[] = []
    let m: RegExpExecArray | null
    tagRe.lastIndex = 0
    while ((m = tagRe.exec(raw)) !== null) {
      const min = parseInt(m[1], 10) || 0
      const sec = parseFloat(String(m[2]).replace(':', '.')) || 0
      stamps.push(min * 60 + sec)
    }
    if (!stamps.length) continue
    const text = raw.replace(tagRe, '').trim()
    if (!text) continue
    for (const s of stamps) out.push({ time: s, text })
  }
  out.sort((a, b) => a.time - b.time)
  // 去重相邻重复时间戳
  const dedup: LyricLine[] = []
  for (const line of out) {
    const prev = dedup[dedup.length - 1]
    if (prev && Math.abs(prev.time - line.time) < 0.001 && prev.text === line.text) continue
    dedup.push(line)
  }
  return dedup
}

async function fetchLrclib(artist: string, title: string, durationSec?: number, album?: string): Promise<{ lrc: string, durationMs?: number, album?: string } | null> {
  if (!title) return null
  try {
    const params: Record<string, string> = {
      track_name: title
    }
    if (artist) params.artist_name = artist
    if (album) params.album_name = album
    if (durationSec) params.duration = String(Math.round(durationSec))
    let resp = await axios.get('https://lrclib.net/api/get', {
      params,
      timeout: 8000,
      headers: { 'User-Agent': 'BoxPlayer/1.0 (https://github.com/gaozhangmin/boxplayer)' }
    }).catch(() => null)
    let body: any = resp?.data
    if (!body || (!body.syncedLyrics && !body.plainLyrics)) {
      // 退化使用 search 接口
      const sResp = await axios.get('https://lrclib.net/api/search', {
        params: { track_name: title, ...(artist ? { artist_name: artist } : {}) },
        timeout: 8000,
        headers: { 'User-Agent': 'BoxPlayer/1.0' }
      }).catch(() => null)
      const list: any[] = Array.isArray(sResp?.data) ? sResp!.data : []
      const best = list.find((item: any) => item?.syncedLyrics) || list[0]
      if (best) body = best
    }
    if (!body) return null
    const lrc: string = body.syncedLyrics || body.plainLyrics || ''
    if (!lrc) return null
    return {
      lrc,
      durationMs: body.duration ? Math.round(Number(body.duration) * 1000) : undefined,
      album: body.albumName
    }
  } catch (e) {
    DebugLog.mSaveWarning('LRCLIB error: ' + (e as Error).message)
    return null
  }
}

async function fetchITunes(artist: string, title: string): Promise<{ cover: string, album: string, artist: string, title: string } | null> {
  if (!title) return null
  try {
    const term = (artist ? artist + ' ' : '') + title
    const resp = await axios.get('https://itunes.apple.com/search', {
      params: { term, entity: 'song', limit: 5, country: 'us' },
      timeout: 8000
    }).catch(() => null)
    const results: any[] = resp?.data?.results || []
    if (!results.length) return null
    const lower = (s: any) => String(s || '').toLowerCase()
    const targetTitle = lower(title)
    const targetArtist = lower(artist)
    const score = (r: any) => {
      let s = 0
      if (lower(r.trackName) === targetTitle) s += 4
      else if (lower(r.trackName).includes(targetTitle)) s += 2
      if (targetArtist && lower(r.artistName) === targetArtist) s += 3
      else if (targetArtist && lower(r.artistName).includes(targetArtist)) s += 1
      return s
    }
    results.sort((a, b) => score(b) - score(a))
    const r = results[0]
    if (!r) return null
    let cover: string = r.artworkUrl100 || r.artworkUrl60 || ''
    if (cover) cover = cover.replace(/\/\d+x\d+bb\.(jpg|png)$/i, '/600x600bb.jpg')
    return {
      cover,
      album: r.collectionName || '',
      artist: r.artistName || artist,
      title: r.trackName || title
    }
  } catch (e) {
    DebugLog.mSaveWarning('iTunes error: ' + (e as Error).message)
    return null
  }
}

export async function fetchMusicMetadata(q: MetaQuery): Promise<MusicMetadata> {
  const guess = guessArtistTitle(q.filename)
  const artist = (q.artistHint || guess.artist || '').trim()
  const title = (q.titleHint || guess.title || '').trim()
  const album = (q.albumHint || '').trim()
  const empty = (): MusicMetadata => ({
    title: title || guess.title,
    artist: artist || guess.artist,
    album,
    cover: '',
    lrc: '',
    lines: []
  })
  if (!q.filename && !title) return empty()
  const key = buildKey(q)
  const now = Date.now()
  const cached = memCache.get(key)
  if (cached && now - cached.ts < META_TTL_MS) {
    return { ...cached.data, fromCache: true }
  }
  const existing = inflight.get(key)
  if (existing) return existing

  const promise = (async () => {
    const [lrcRes, itunesRes] = await Promise.all([
      fetchLrclib(artist, title, q.durationSec, album || undefined),
      fetchITunes(artist, title)
    ])
    const data: MusicMetadata = {
      title: itunesRes?.title || title || guess.title,
      artist: itunesRes?.artist || artist || guess.artist,
      album: itunesRes?.album || lrcRes?.album || album || '',
      cover: itunesRes?.cover || '',
      durationMs: lrcRes?.durationMs,
      lrc: lrcRes?.lrc || '',
      lines: parseLrc(lrcRes?.lrc || '')
    }
    memCache.set(key, { ts: Date.now(), data })
    return data
  })().finally(() => {
    inflight.delete(key)
  })
  inflight.set(key, promise)
  return promise
}

export function findActiveLineIndex(lines: LyricLine[], currentSec: number): number {
  if (!lines.length) return -1
  if (currentSec < lines[0].time) return -1
  let lo = 0
  let hi = lines.length - 1
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1
    if (lines[mid].time <= currentSec) lo = mid
    else hi = mid - 1
  }
  return lo
}

export function clearMusicMetaCache(): void {
  memCache.clear()
  inflight.clear()
}
