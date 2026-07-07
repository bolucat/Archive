/**
 * Kugou (kg) SDK — search, lyric, cover art only.
 * No getMusicUrl — copyright-safe.
 */
import { httpFetch, type RequestObj } from '../request'
import type { OldMusicInfo } from '../wy/index'

const { inflate } = require('zlib')

function decodeName(str: string): string {
  return str
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&apos;|&#039;/g, "'")
    .replace(/&nbsp;/g, ' ')
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

function formatSingerName(singers: any[], nameField: string): string {
  return (singers || []).map((s) => s[nameField]).join('、')
}

// ---- KRC Decoder ----

const enc_key = Buffer.from([
  0x40, 0x47, 0x61, 0x77, 0x5e, 0x32, 0x74, 0x47, 0x51, 0x36, 0x31, 0x2d, 0xce, 0xd2, 0x6e, 0x69,
])

function decodeKrc(base64Data: string): Promise<{
  lyric: string
  tlyric: string
  rlyric: string
  lxlyric: string
}> {
  return new Promise((resolve, reject) => {
    if (!base64Data) {
      resolve({ lyric: '', tlyric: '', rlyric: '', lxlyric: '' })
      return
    }
    const buf = Buffer.from(base64Data, 'base64').subarray(4)
    for (let i = 0; i < buf.length; i++) {
      buf[i] = buf[i] ^ enc_key[i % 16]
    }
    inflate(buf, (err: Error | null, result: Buffer) => {
      if (err) return reject(err)
      resolve(parseKrc(result.toString()))
    })
  })
}

function parseKrc(str: string) {
  str = str.replace(/\r/g, '')
  if (/^.*\[id:\$\w+\]\n/.test(str)) {
    str = str.replace(/^.*\[id:\$\w+\]\n/, '')
  }

  let tlyric = ''
  let rlyric = ''
  const transMatch = str.match(/\[language:([\w=\\/+]+)\]/)
  if (transMatch) {
    str = str.replace(/\[language:[\w=\\/+]+\]\n/, '')
    try {
      const json = JSON.parse(Buffer.from(transMatch[1], 'base64').toString())
      for (const item of json.content || []) {
        if (item.type === 0 && item.lyricContent) rlyric = item.lyricContent.join('\n')
        if (item.type === 1 && item.lyricContent) tlyric = item.lyricContent.join('\n')
      }
    } catch (_) {}
  }

  let lxlyric = str.replace(/\[((\d+),\d+)\].*/g, (match) => {
    const result = match.match(/\[((\d+),\d+)\].*/)
    return result ? match.replace(/^\[[\d,]+/, '[') : match
  })

  lxlyric = lxlyric.replace(/<(\d+,\d+),\d+>/g, '<$1>')
  lxlyric = decodeName(lxlyric)
  const lyric = lxlyric.replace(/<\d+,\d+>/g, '')
  return { lyric, tlyric: decodeName(tlyric), rlyric: decodeName(rlyric), lxlyric }
}

// ---- Search ----

function filterData(rawData: any): OldMusicInfo | null {
  const types: Array<{ type: string; size: string | null }> = []
  const _types: Record<string, { size: string | null }> = {}
  if (rawData.FileSize !== 0) {
    const size = sizeFormate(rawData.FileSize)
    types.push({ type: '128k', size })
    _types['128k'] = { size }
  }
  if (rawData.HQFileSize !== 0) {
    const size = sizeFormate(rawData.HQFileSize)
    types.push({ type: '320k', size })
    _types['320k'] = { size }
  }
  if (rawData.SQFileSize !== 0) {
    const size = sizeFormate(rawData.SQFileSize)
    types.push({ type: 'flac', size })
    _types.flac = { size }
  }
  return {
    singer: decodeName(formatSingerName(rawData.Singers || [], 'name')),
    name: decodeName(rawData.SongName || ''),
    albumName: decodeName(rawData.AlbumName || ''),
    albumId: rawData.AlbumID,
    songmid: rawData.Audioid,
    source: 'kg',
    interval: formatPlayTime(rawData.Duration || 0),
    _interval: rawData.Duration,
    img: null,
    lrc: null,
    otherSource: null,
    hash: rawData.FileHash,
    types,
    _types,
    typeUrl: {},
  }
}

function handleSearchResult(rawData: any): OldMusicInfo[] {
  const ids = new Set<string>()
  const list: OldMusicInfo[] = []
  for (const item of rawData || []) {
    const key = item.Audioid + item.FileHash
    if (ids.has(key)) continue
    ids.add(key)
    const result = filterData(item)
    if (result) list.push(result)
    for (const child of item.Grp || []) {
      const childKey = item.Audioid + child.FileHash
      if (ids.has(childKey)) continue
      ids.add(childKey)
      const childResult = filterData(child)
      if (childResult) list.push(childResult)
    }
  }
  return list
}

export interface SearchResult {
  list: OldMusicInfo[]
  allPage: number
  limit: number
  total: number
  source: string
}

export function search(str: string, page = 1, limit = 30): RequestObj {
  const requestObj = httpFetch(
    `https://songsearch.kugou.com/song_search_v2?keyword=${encodeURIComponent(str)}&page=${page}&pagesize=${limit}&userid=0&clientver=&platform=WebFilter&filter=2&iscorrection=1&privilege_filter=0&area_code=1`
  )

  requestObj.promise = requestObj.promise.then((resp: any) => {
    const body = resp.body
    if (!body || body.error_code !== 0) return Promise.reject(new Error('搜索失败'))
    const list = handleSearchResult(body.data?.lists)
    return {
      list,
      allPage: Math.ceil((body.data?.total || 0) / limit),
      limit,
      total: body.data?.total || 0,
      source: 'kg',
    }
  })

  return requestObj
}

// ---- Lyric ----

function getIntv(interval: string): number {
  if (!interval) return 0
  const parts = interval.split(':')
  let result = 0
  let unit = 1
  while (parts.length) {
    result += parseInt(parts.pop()!) * unit
    unit *= 60
  }
  return result
}

interface LyricResult {
  lyric: string
  tlyric: string
  rlyric: string
  lxlyric: string
}

export function getLyric(songInfo: any): RequestObj {
  const name = songInfo.name || ''
  const hash = songInfo.hash || ''
  const time = songInfo._interval || getIntv(songInfo.interval || '0')

  const searchObj = httpFetch(
    `http://lyrics.kugou.com/search?ver=1&man=yes&client=pc&keyword=${encodeURIComponent(name)}&hash=${hash}&timelength=${time}&lrctxt=1`,
    {
      headers: {
        'KG-RC': '1',
        'KG-THash': 'expand_search_manager.cpp:852736169:451',
        'User-Agent': 'KuGou2012-9020-ExpandSearchManager',
      },
    }
  )

  searchObj.promise = searchObj.promise.then((resp: any) => {
    if (resp.statusCode !== 200 || !resp.body?.candidates?.length) {
      return Promise.reject(new Error('歌词搜索失败'))
    }
    const info = resp.body.candidates[0]
    const fmt = info.krctype === 1 && info.contenttype !== 1 ? 'krc' : 'lrc'
    return downloadLyric(info.id, info.accesskey, fmt)
  })

  return searchObj
}

function downloadLyric(id: string, accesskey: string, fmt: string): Promise<LyricResult> {
  const dlObj = httpFetch(
    `http://lyrics.kugou.com/download?ver=1&client=pc&id=${id}&accesskey=${accesskey}&fmt=${fmt}&charset=utf8`,
    {
      headers: {
        'KG-RC': '1',
        'KG-THash': 'expand_search_manager.cpp:852736169:451',
        'User-Agent': 'KuGou2012-9020-ExpandSearchManager',
      },
    }
  )

  return dlObj.promise.then((resp: any) => {
    if (resp.statusCode !== 200) return Promise.reject(new Error('歌词下载失败'))
    if (resp.body.fmt === 'krc') {
      return decodeKrc(resp.body.content)
    } else {
      return {
        lyric: Buffer.from(resp.body.content, 'base64').toString('utf-8'),
        tlyric: '',
        rlyric: '',
        lxlyric: '',
      }
    }
  })
}

// ---- Cover (kg doesn't have a direct cover API, return empty) ----

export function getPic(_songInfo: any): RequestObj {
  return {
    promise: Promise.resolve(''),
    cancelHttp: () => {},
    isCancelled: false,
  }
}

const kg = {
  search,
  getLyric,
  getPic,
}

export default kg
