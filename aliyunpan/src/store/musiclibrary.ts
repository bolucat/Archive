import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { IMusicTrack } from '../types/music'
import DB from '../utils/db'

const LS_AUTOSCAN = 'musicLibrary.autoScan'
const LS_LASTSCAN = 'musicLibrary.lastScanAt'
const LS_SUBTAB = 'musicLibrary.subTab'

export type MusicSubTab = 'home' | 'all' | 'artists' | 'albums' | 'folders' | 'fav'

const ARTIST_TITLE_RE = /^(.+?)\s*[-–—_]\s*(.+)$/
const COMMON_BRACKETS = /[\(\[（【][^\)\]）】]*[\)\]）】]/g
const TRACK_NUM_PREFIX_RE = /^\s*(?:CD\s*\d+\s*[-_.]\s*)?\d+\s*[.\-_、)]\s*/i

function stripExt(name: string): string {
  if (!name) return ''
  const i = name.lastIndexOf('.')
  return i > 0 ? name.slice(0, i) : name
}

function stripTrackNumber(s: string): string {
  if (!s) return s
  return s.replace(TRACK_NUM_PREFIX_RE, '').trim()
}

function parseArtistTitle(file_name: string): { artist: string; title: string } {
  const baseRaw = stripExt(file_name).replace(COMMON_BRACKETS, ' ').replace(/\s+/g, ' ').trim()
  if (!baseRaw) return { artist: '', title: '' }
  const base = stripTrackNumber(baseRaw)
  const m = base.match(ARTIST_TITLE_RE)
  if (m && m[1] && m[2]) {
    return { artist: stripTrackNumber(m[1].trim()), title: m[2].trim() }
  }
  return { artist: '', title: base }
}

function ensureArtistTitle(t: IMusicTrack): IMusicTrack {
  // 修复历史脏数据：artist 带轨道号前缀（如 "01. 周杰倫"）的重新解析
  const looksDirty = t.artist && TRACK_NUM_PREFIX_RE.test(t.artist)
  if (t.artist && t.title && !looksDirty) return t
  const { artist, title } = parseArtistTitle(t.file_name)
  if (!t.artist || looksDirty) t.artist = artist
  if (!t.title) t.title = title
  return t
}

function loadJson<T>(key: string, fallback: T): T {
  try {
    const s = localStorage.getItem(key)
    if (!s) return fallback
    const parsed = JSON.parse(s)
    return parsed === null || parsed === undefined ? fallback : parsed
  } catch {
    return fallback
  }
}

function saveJson(key: string, value: unknown) {
  try { localStorage.setItem(key, JSON.stringify(value)) } catch {}
}

function mulberry32(seed: number) {
  let a = seed >>> 0
  return function () {
    a = (a + 0x6D2B79F5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const useMusicLibraryStore = defineStore('musiclibrary', () => {
  const tracks = ref<IMusicTrack[]>([])
  const loaded = ref(false)
  const isScanning = ref(false)
  const scanLabel = ref('')
  const scanScanned = ref(0)
  const scanFound = ref(0)
  const scanError = ref('')
  const lastScanAt = ref<number>(loadJson<number>(LS_LASTSCAN, 0))
  const autoScanEnabled = ref<boolean>(loadJson<boolean>(LS_AUTOSCAN, true))
  const randomSeed = ref<number>(Date.now())
  const subTab = ref<MusicSubTab>(loadJson<MusicSubTab>(LS_SUBTAB, 'home'))

  const totalCount = computed(() => tracks.value.length)

  const recentlyAdded = computed<IMusicTrack[]>(() => {
    return [...tracks.value]
      .sort((a, b) => (b.scanned_at || 0) - (a.scanned_at || 0))
      .slice(0, 30)
  })

  const randomPicks = computed<IMusicTrack[]>(() => {
    const arr = tracks.value
    if (!arr.length) return []
    const rand = mulberry32(randomSeed.value)
    const indices = arr.map((_, i) => i)
    for (let i = indices.length - 1; i > 0; i--) {
      const j = Math.floor(rand() * (i + 1))
      ;[indices[i], indices[j]] = [indices[j], indices[i]]
    }
    return indices.slice(0, 24).map((i) => arr[i])
  })

  const byArtist = computed(() => {
    const map = new Map<string, IMusicTrack[]>()
    for (const t of tracks.value) {
      const key = (t.artist || '').trim() || '未知艺人'
      const arr = map.get(key)
      if (arr) arr.push(t)
      else map.set(key, [t])
    }
    return Array.from(map.entries())
      .map(([artist, items]) => ({ artist, items, count: items.length }))
      .sort((a, b) => b.count - a.count)
  })

  const byAlbum = computed(() => {
    const map = new Map<string, IMusicTrack[]>()
    for (const t of tracks.value) {
      const key = (t.album || '').trim() || '未知专辑'
      const arr = map.get(key)
      if (arr) arr.push(t)
      else map.set(key, [t])
    }
    return Array.from(map.entries())
      .map(([album, items]) => ({ album, items, count: items.length }))
      .sort((a, b) => b.count - a.count)
  })

  const byFolder = computed(() => {
    const map = new Map<string, IMusicTrack[]>()
    for (const t of tracks.value) {
      const key = (t.parent_path || t.parent_file_id || '').trim() || '未分组'
      const arr = map.get(key)
      if (arr) arr.push(t)
      else map.set(key, [t])
    }
    return Array.from(map.entries())
      .map(([path, items]) => ({
        path,
        name: path.split('/').pop() || path,
        items,
        count: items.length,
        scanned_at: items.reduce((m, t) => Math.max(m, t.scanned_at || 0), 0)
      }))
      .sort((a, b) => b.scanned_at - a.scanned_at)
  })

  const favoritesTracks = computed<IMusicTrack[]>(() => {
    try {
      const favs: any[] = JSON.parse(localStorage.getItem('pageMusic.favorites') || '[]') || []
      const ids = new Set<string>(favs.map((f) => `${f.user_id || ''}|${f.drive_id || ''}|${f.file_id || ''}`))
      const found = tracks.value.filter((t) => ids.has(t.id))
      // 已收藏但不在库里的（可能是其它来源），用 favs 自身合成
      const present = new Set(found.map((t) => t.id))
      const synthetic: IMusicTrack[] = []
      for (const f of favs) {
        const id = `${f.user_id || ''}|${f.drive_id || ''}|${f.file_id || ''}`
        if (!present.has(id)) {
          synthetic.push(ensureArtistTitle({
            id,
            user_id: f.user_id || '',
            drive_id: f.drive_id || '',
            file_id: f.file_id || '',
            parent_file_id: f.parent_file_id || '',
            file_name: f.file_name || '',
            ext: f.ext || '',
            size: f.size || 0,
            category: f.category || 'audio',
            thumbnail: f.thumbnail || '',
            description: f.description || '',
            encType: f.encType || '',
            scanned_at: 0
          }))
        }
      }
      return [...found, ...synthetic]
    } catch {
      return []
    }
  })

  async function loadFromDB() {
    if (loaded.value) return
    try {
      const list = await DB.getAllMusicTracks()
      const dirtyToFix: IMusicTrack[] = []
      const fixed = list.map((raw) => {
        const before = { artist: raw.artist, title: raw.title }
        const t = ensureArtistTitle(raw)
        if (t.artist !== before.artist || t.title !== before.title) {
          // 文本修正过 → 之前 enrich 的结果基于脏字段，清掉重跑
          t.enriched_at = undefined
          if (!t.cover_url) {
            // 没有封面的也清，让 enrich 用新 artist/title 重试
          }
          dirtyToFix.push(t)
        }
        return t
      })
      tracks.value = fixed
      loaded.value = true
      if (dirtyToFix.length) {
        DB.saveMusicTracks(dirtyToFix).catch(() => {})
      }
    } catch (e) {
      console.warn('musicLibrary loadFromDB failed', e)
    }
  }

  function setScanProgress(label: string, scanned: number, found: number) {
    scanLabel.value = label
    scanScanned.value = scanned
    scanFound.value = found
  }

  async function appendTracks(newTracks: IMusicTrack[]) {
    if (!newTracks.length) return
    const enriched = newTracks.map(ensureArtistTitle)
    await DB.saveMusicTracks(enriched).catch(() => {})
    // 内存合并：按 id 去重
    const map = new Map<string, IMusicTrack>()
    for (const t of tracks.value) map.set(t.id, t)
    for (const t of enriched) map.set(t.id, t)
    tracks.value = Array.from(map.values())
  }

  function removeTracksByIds(ids: string[]) {
    if (!ids || ids.length === 0) return
    const removeSet = new Set(ids)
    tracks.value = tracks.value.filter((t) => !removeSet.has(t.id))
  }

  async function updateTrackEnrichment(id: string, patch: Partial<IMusicTrack>) {
    const idx = tracks.value.findIndex((t) => t.id === id)
    if (idx < 0) return
    const merged: IMusicTrack = {
      ...tracks.value[idx],
      ...patch,
      id: tracks.value[idx].id,
      enriched_at: Date.now()
    }
    // 触发响应式
    tracks.value = [
      ...tracks.value.slice(0, idx),
      merged,
      ...tracks.value.slice(idx + 1)
    ]
    DB.saveMusicTracks([merged]).catch(() => {})
  }

  function setIsScanning(v: boolean, errMsg = '') {
    isScanning.value = v
    if (!v) {
      scanLabel.value = ''
      scanScanned.value = 0
      scanFound.value = 0
    }
    scanError.value = errMsg
  }

  function markScanFinished() {
    lastScanAt.value = Date.now()
    saveJson(LS_LASTSCAN, lastScanAt.value)
  }

  function rerollRandom() {
    randomSeed.value = Date.now()
  }

  function setAutoScan(v: boolean) {
    autoScanEnabled.value = v
    saveJson(LS_AUTOSCAN, v)
  }

  function setSubTab(t: MusicSubTab) {
    subTab.value = t
    saveJson(LS_SUBTAB, t)
  }

  async function clearAll() {
    await DB.clearMusicTracks().catch(() => {})
    tracks.value = []
    lastScanAt.value = 0
    saveJson(LS_LASTSCAN, 0)
  }

  return {
    tracks,
    loaded,
    isScanning,
    scanLabel,
    scanScanned,
    scanFound,
    scanError,
    lastScanAt,
    autoScanEnabled,
    randomSeed,
    subTab,
    totalCount,
    recentlyAdded,
    randomPicks,
    byArtist,
    byAlbum,
    byFolder,
    favoritesTracks,
    loadFromDB,
    setScanProgress,
    appendTracks,
    removeTracksByIds,
    updateTrackEnrichment,
    setIsScanning,
    markScanFinished,
    rerollRandom,
    setAutoScan,
    setSubTab,
    clearAll
  }
})

export default useMusicLibraryStore
export { parseArtistTitle, ensureArtistTitle, stripExt }
