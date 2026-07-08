/**
 * NetEase Cloud Music (wy) SDK — search, lyric, cover art only.
 * No getMusicUrl — copyright-safe.
 */
import { httpFetch, type RequestObj } from '../request'
import { weapi, eapi } from './crypto'

function eapiRequest(url: string, data: Record<string, any>) {
  return httpFetch('http://interface.music.163.com/eapi/batch', {
    method: 'post',
    headers: {
      'User-Agent':
        'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/60.0.3112.90 Safari/537.36',
      origin: 'https://music.163.com',
    },
    form: eapi(url, data),
  })
}

function weapiRequest(url: string, data: Record<string, any>) {
  const form = weapi(data)
  return httpFetch(url, {
    method: 'post',
    headers: {
      'User-Agent':
        'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/60.0.3112.90 Safari/537.36',
      Referer: 'https://music.163.com',
      origin: 'https://music.163.com',
    },
    form,
  })
}

function sizeFormate(size: number): string {
  if (!size) return ''
  if (size < 1024) return size.toFixed(1) + 'B'
  if (size < 1024 * 1024) return (size / 1024).toFixed(1) + 'K'
  return (size / (1024 * 1024)).toFixed(1) + 'M'
}

function formatPlayTime(sec: number): string {
  const m = Math.floor(sec / 60).toString().padStart(2, '0')
  const s = Math.floor(sec % 60).toString().padStart(2, '0')
  return `${m}:${s}`
}

// ---- Music Info (old format) ----

export interface OldMusicInfo {
  singer: string
  name: string
  albumName: string
  albumId: number
  source: string
  interval: string
  songmid: number | string
  img: string | null
  lrc: string | null
  types: Array<{ type: string; size: string | null }>
  _types: Record<string, { size: string | null }>
  typeUrl: Record<string, any>
  hash?: string
  _interval?: number
  otherSource?: any
}

// ---- Lyric ----

interface LyricResult {
  lyric: string
  tlyric: string
  rlyric: string
  lxlyric: string
}

export function getLyric(songmid: string | number): RequestObj {
  const requestObj = eapiRequest('/api/song/lyric/v1', {
    id: songmid,
    cp: false,
    tv: 0,
    lv: 0,
    rv: 0,
    kv: 0,
    yv: 0,
    ytv: 0,
    yrv: 0,
  })

  requestObj.promise = requestObj.promise.then((resp: any) => {
    const body = resp.body
    if (body.code !== 200 || !body?.lrc?.lyric) return Promise.reject(new Error('获取歌词失败'))
    return parseLyricResult(body)
  })

  return requestObj
}

function fixTimeLabel(lrc: string): string {
  return lrc.replace(/\[(\d{2}:\d{2}):(\d{2})\]/g, '[$1.$2]')
}

function parseLyricResult(body: any): LyricResult {
  const lrc = fixTimeLabel(body.lrc?.lyric || '')
  const tlyric = body.tlyric?.lyric ? fixTimeLabel(body.tlyric.lyric) : ''
  const rlyric = body.romalrc?.lyric ? fixTimeLabel(body.romalrc.lyric) : ''
  const lxlyric = parseEnhancedLyric(body.yrc?.lyric)
  return { lyric: lrc, tlyric, rlyric, lxlyric }
}

function parseEnhancedLyric(ylrc: string | undefined): string {
  if (!ylrc) return ''
  const lines = ylrc.trim().replace(/\r/g, '').split('\n')
  const result: string[] = []
  for (const line of lines) {
    const timeMatch = /^\[(\d+),\d+\]/.exec(line)
    if (!timeMatch) {
      if (line.startsWith('[offset')) result.push(line)
      continue
    }
    const startMs = parseInt(timeMatch[1])
    const m = Math.floor(startMs / 60000).toString().padStart(2, '0')
    const s = Math.floor((startMs % 60000) / 1000).toString().padStart(2, '0')
    const ms = (startMs % 1000).toString()
    const timeStr = `[${m}:${s}.${ms}]`
    const wordTimeAll = /(\(\d+,\d+,\d+\))/g
    const words = line.replace(/^\[\d+,\d+\]/, '')
    const times = words.match(wordTimeAll)
    if (!times) {
      result.push(`${timeStr}${words.replace(wordTimeAll, '')}`)
      continue
    }
    const wordParts = words.split(/\(\d+,\d+,\d+\)/)
    wordParts.shift()
    const newWords = times
      .map((time, i) => {
        const m2 = /\((\d+),(\d+),\d+\)/.exec(time)
        if (!m2) return ''
        return `<${Math.max(parseInt(m2[1]) - startMs, 0)},${m2[2]}>${wordParts[i] || ''}`
      })
      .join('')
    result.push(`${timeStr}${newWords}`)
  }
  return result.join('\n')
}

// ---- Pic / Cover ----

export function getPic(songmid: string | number): RequestObj {
  const requestObj = weapiRequest('https://music.163.com/weapi/v3/song/detail', {
    c: `[{"id":${songmid}}]`,
    ids: `[${songmid}]`,
  })

  requestObj.promise = requestObj.promise.then((resp: any) => {
    const body = resp.body
    if (body.code !== 200 || !body.songs?.length) return Promise.reject(new Error('获取歌曲信息失败'))
    return body.songs[0].al?.picUrl || ''
  })

  return requestObj
}

// ---- Search ----

export interface SearchResult {
  list: OldMusicInfo[]
  allPage: number
  limit: number
  total: number
  source: string
}

export function search(str: string, page = 1, limit = 30): RequestObj {
  const requestObj = eapiRequest('/api/search/song/list/page', {
    keyword: str,
    needCorrect: '1',
    channel: 'typing',
    offset: limit * (page - 1),
    scene: 'normal',
    total: page === 1,
    limit,
  })

  requestObj.promise = requestObj.promise.then((resp: any) => {
    const body = resp.body
    if (body.code !== 200) return Promise.reject(new Error('搜索失败'))
    const resources = body.result?.data?.resources || body.data?.resources || []
    const list = handleSearchResult(resources)
    const total = body.result?.data?.totalCount || body.data?.totalCount || 0
    const allPage = Math.ceil(total / limit)
    return { list, allPage, limit, total, source: 'wy' }
  })

  return requestObj
}

function handleSearchResult(rawList: any[]): OldMusicInfo[] {
  if (!rawList) return []
  return rawList
    .map((item) => {
      const data = item.baseInfo?.simpleSongData || item
      if (!data.name) return null
      const types: Array<{ type: string; size: string | null }> = []
      const _types: Record<string, { size: string | null }> = {}
      const priv = data.privilege || {}

      if (priv.maxBrLevel === 'hires') {
        const sz = data.hr ? sizeFormate(data.hr.size) : null
        types.push({ type: 'flac24bit', size: sz })
        _types.flac24bit = { size: sz }
      }
      switch (priv.maxbr) {
        case 999000:
          const sqSize = data.sq ? sizeFormate(data.sq.size) : null
          types.push({ type: 'flac', size: sqSize })
          _types.flac = { size: sqSize }
        case 320000:
          const hSize = data.h ? sizeFormate(data.h?.size) : null
          types.push({ type: '320k', size: hSize })
          _types['320k'] = { size: hSize }
        case 192000:
        case 128000:
          const lSize = data.l ? sizeFormate(data.l?.size) : null
          types.push({ type: '128k', size: lSize })
          _types['128k'] = { size: lSize }
      }
      types.reverse()

      return {
        singer: (data.ar || []).map((s: any) => s.name).join('、'),
        name: data.name,
        albumName: data.al?.name || '',
        albumId: data.al?.id || 0,
        source: 'wy',
        interval: formatPlayTime((data.dt || 0) / 1000),
        _interval: Math.round((data.dt || 0) / 1000),
        songmid: data.id,
        img: data.al?.picUrl || null,
        lrc: null,
        types,
        _types,
        typeUrl: {},
      } as OldMusicInfo
    })
    .filter(Boolean) as OldMusicInfo[]
}

const wy = {
  search,
  getLyric,
  getPic,
}

export default wy
