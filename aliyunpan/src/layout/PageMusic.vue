<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { KeyboardState, useAppStore, useKeyboardStore } from '../store'
import useMusicPlayerStore, { type MusicPlayerState } from '../store/musicplayerstore'
import message from '../utils/message'
import { TestAlt, TestKey, TestShift } from '../utils/keyboardhelper'
import { getRawUrl } from '../utils/proxyhelper'
import type { IPageMusicTrack } from '../store/appstore'
import { fetchMusicMetadata, findActiveLineIndex, parseLrc, type LyricLine, type MusicMetadata } from '../utils/musicMetadata'
import { bindAudio, ensureInit as initAudioEngine } from '../module/audioplayer/index'
import { shouldHandleMusicPlayerDblClick } from '../utils/pageMusicEvents'
import { loadMusicTrackList, musicTrackKey, saveMusicTrackList } from '../utils/musicPlayerStorage'
import { RotateCcw } from 'lucide-vue-next'
import MusicImmersiveStage from './music/MusicImmersiveStage.vue'
import MusicBottomConsole from './music/MusicBottomConsole.vue'
import MusicMiniQueue from './music/MusicMiniQueue.vue'
import { fadeTo } from '../module/audioplayer/index'
import MusicQueuePanel from './music/MusicQueuePanel.vue'
import CustomLyricModal from '../components/radio/CustomLyricModal.vue'
import CoverCropModal from '../components/radio/CoverCropModal.vue'
import { DEFAULT_MUSIC_FX, MUSIC_FX_PRESETS, normalizeMusicFxConfig, parseMusicFxConfigJson, type MusicFxConfig } from '../utils/radio/FxConfig'
import type { ShelfCard } from '../utils/radio/ShelfManager'
import { deleteMineradioValue, getMineradioValue, setMineradioValue } from '../utils/radio/MineradioStorage'
import { extractCoverPalette, type CoverPalette } from '../utils/radio/CoverColorExtractor'
import { analyzeAudioElementBeat } from '../utils/radio/BeatAnalyzer'
import { getCachedBeatMap, type CachedBeatMap } from '../utils/radio/BeatMapCache'
import { DEFAULT_MINERADIO_HOTKEYS, hasHotkeyConflict, hotkeyFromEvent, loadHotkeys, saveHotkeys, type MineradioHotkey } from '../utils/radio/HotkeyConfig'
import { addTracksToList, createPlaylist, loadPlaylists, savePlaylists, type LocalPlaylist } from '../utils/radio/LocalPlaylistManager'

const props = defineProps<{ embedded?: boolean; sidePanel?: boolean }>()
const emit = defineEmits<{ (e: 'state-change', state: MusicPlayerState): void }>()

const appStore = useAppStore()
const keyboardStore = useKeyboardStore()
const musicPlayerStore = useMusicPlayerStore()

keyboardStore.$subscribe((_m: any, state: KeyboardState) => {
  if (TestAlt('f4', state.KeyDownEvent, handleHide)) return
  if (TestAlt('m', state.KeyDownEvent, handleMin)) return
  if (TestAlt('enter', state.KeyDownEvent, handleMax)) return
  if (TestKey('f11', state.KeyDownEvent, handleMax)) return
  if (TestKey(' ', state.KeyDownEvent, togglePlay)) return
  if (TestShift('arrowright', state.KeyDownEvent, () => playNext(false))) return
  if (TestShift('arrowleft', state.KeyDownEvent, playPrev)) return
  if (TestKey('arrowright', state.KeyDownEvent, () => seekRel(5))) return
  if (TestKey('arrowleft', state.KeyDownEvent, () => seekRel(-5))) return
  if (TestKey('escape', state.KeyDownEvent, () => { immersiveMode.value = false; controlsVisible.value = true })) return
  if (TestKey('arrowup', state.KeyDownEvent, () => setVol(Math.min(1, vol.value + 0.05)))) return
  if (TestKey('arrowdown', state.KeyDownEvent, () => setVol(Math.max(0, vol.value - 0.05)))) return
})

const onKeyDown = (e: KeyboardEvent) => {
  const el = (e.target || e.srcElement) as any
  if (document.querySelector('.arco-modal-container')) return
  if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return
  if (e.key.toLowerCase() === 'escape' && (showVisualGuide.value || showHotkeyModal.value || showBeatModal.value || showTrackDetail.value || showCustomLyric.value || showCoverCrop.value || hotkeyCaptureId.value)) {
    e.preventDefault()
    showVisualGuide.value = false
    showHotkeyModal.value = false
    showBeatModal.value = false
    showTrackDetail.value = false
    showCustomLyric.value = false
    showCoverCrop.value = false
    hotkeyCaptureId.value = ''
    return
  }
  if (el?.nodeName === 'INPUT' || el?.nodeName === 'TEXTAREA') return
  if (handleCustomHotkey(e)) return
  keyboardStore.KeyDown(e)
}

type PlayMode = 'list' | 'loop-list' | 'loop-one' | 'shuffle'
type PanelTab = 'queue' | 'fav' | 'recent'
interface MusicFxUserSlot {
  id: string
  name: string
  config: MusicFxConfig | null
  updatedAt: number
}

const COLOR_LAB_PRESETS = [
  { label: '青色', color: '#00f5d4' },
  { label: '香槟', color: '#f4d28a' },
  { label: '霓虹', color: '#ff7aa2' },
  { label: '冰蓝', color: '#7fd8ff' },
  { label: '紫电', color: '#b78cff' },
  { label: '薄荷', color: '#9cffdf' },
  { label: '日冕', color: '#fff0b8' },
  { label: '暗红', color: '#ff5c6c' }
]

const audioRef = ref<HTMLAudioElement | null>(null)
const playlist = ref<IPageMusicTrack[]>([])
const curIdx = ref(0)
const playing = ref(false)
const loading = ref(false)
const curTime = ref(0)
const dur = ref(0)
const vol = ref(0.8)
const muted = ref(false)
const mode = ref<PlayMode>('loop-list')
const showLyrics = ref(false)
const showPanel = ref(false)
const showQueue = ref(false)
const panelTab = ref<PanelTab>('queue')
const errMsg = ref('')
const seekPreview = ref<number | null>(null)
const shuffleHist = ref<number[]>([])
const favs = ref<IPageMusicTrack[]>([])
const recents = ref<IPageMusicTrack[]>([])
const listFilter = ref('')
const showFxPanel = ref(false)
const showCustomLyric = ref(false)
const showCoverCrop = ref(false)
const showTrackDetail = ref(false)
const showVisualGuide = ref(false)
const showHotkeyModal = ref(false)
const showBeatModal = ref(false)
const showCollectModal = ref(false)
const hotkeyCaptureId = ref('')
const globalHotkeysEnabled = ref(false)
const globalHotkeyStatus = ref('')
const showSplash = ref(false)
const desktopLyricsOn = ref(false)
const lyricTimeOffsetSec = ref(0)
const desktopLyricsClickThrough = ref(false)
const desktopLyricsLocked = ref(false)
const desktopLyricFps = ref(60)
const desktopLyricFontSize = ref(40)
const desktopLyricPrimaryColor = ref('#fff9e8')
const desktopLyricSecondaryColor = ref('#d3e5ec')
const desktopLyricOpacity = ref(1)
const desktopLyricY = ref(0.5)
const desktopLyricBgOpacity = ref(0.42)
const desktopLyricShake = ref(0.16)
const desktopLyricFontFamily = ref('system')
const desktopLyricHighlightFollow = ref(true)
const diySimpleMode = ref(false)
const immersiveMode = ref(false)
const controlsVisible = ref(true)
const customCoverUrl = ref('')
const customBackgroundUrl = ref('')
const customBackgroundType = ref<'image' | 'video'>('image')
const visualFx = ref<MusicFxConfig>({ ...DEFAULT_MUSIC_FX })
const visualFxSlots = ref<MusicFxUserSlot[]>([])
const coverPalette = ref<CoverPalette | null>(null)
const beatAnalyzing = ref(false)
const beatAnalyzingMode = ref<'mr' | 'dj' | null>(null)
const beatInfo = ref<{ mode: 'mr' | 'dj'; bpm: number; peaks: number; updatedAt: number } | null>(null)
const beatMap = ref<CachedBeatMap | null>(null)
const beatChipVisible = ref(false)
const localPlaylists = ref<LocalPlaylist[]>([])
const collectNewPlaylistName = ref('')
const collectTargetTrack = ref<IPageMusicTrack | null>(null)
const gestureHudVisible = ref(false)
const gestureHudText = ref('手势控制已开启：移动鼠标模拟镜头推拉，滚轮切换歌单架。')
const visualGuideIndex = ref(0)
const visualGuideItems = [
  { title: '粒子舞台', text: '封面粒子会跟随实时频谱、BPM 和电影镜头律动。', spot: 'stage' },
  { title: '歌词舞台', text: '点击底部歌词按钮切换，当前歌词会自动居中并同步 3D 字幕。', spot: 'bottom' },
  { title: '3D 歌单架', text: '滚轮切换集合卡，点击中心卡或曲目行即可播放网盘音乐。', spot: 'shelf' },
  { title: '视觉控制台', text: '调节辉光、镜头、歌词、歌单架和封面取色，并可保存到存档槽。', spot: 'fx' }
]
const visualGuideCurrent = computed(() => visualGuideItems[visualGuideIndex.value] || visualGuideItems[0])
const hotkeyItems = ref<MineradioHotkey[]>(DEFAULT_MINERADIO_HOTKEYS.map((item) => ({ ...item })))

let clockRaf = 0
let controlsTimer: number | null = null
let beatChipTimer: number | null = null
let offGlobalHotkey: (() => void) | null = null

const meta = ref<MusicMetadata | null>(null)
const metaLoad = ref(false)
let metaTok = 0
let lastMetaQueryKey = ''
let customLyricAppliedKey = ''

const curTrack = computed(() => playlist.value[curIdx.value])
const title = computed(() => meta.value?.title || stripExt(curTrack.value?.file_name || ''))
const ext = computed(() => {
  const t = curTrack.value
  if (!t) return ''
  if (t.ext) return t.ext.replace(/^\./, '').toUpperCase()
  const d = (t.file_name || '').lastIndexOf('.')
  return d < 0 ? '' : t.file_name!.slice(d + 1).toUpperCase()
})
const coverUrl = computed(() => customCoverUrl.value || meta.value?.cover || curTrack.value?.thumbnail || '')
const artist = computed(() => meta.value?.artist || '')
const album = computed(() => meta.value?.album || '')
const lyricLines = computed<LyricLine[]>(() => meta.value?.lines || [])
const hasLyrics = computed(() => lyricLines.value.length > 0)
const lyricDebugText = computed(() => meta.value?.debug?.reason || '')
const lyricDebugTitle = computed(() => formatLyricDebugTitle(meta.value))
const displayTimeSec = computed(() => seekPreview.value ?? curTime.value)
const lyricClockSec = computed(() => Math.max(0, displayTimeSec.value + lyricTimeOffsetSec.value))
const activeLine = computed(() => findActiveLineIndex(lyricLines.value, lyricClockSec.value))
const desktopLyricText = computed(() => lyricLines.value[activeLine.value]?.text || title.value || curTrack.value?.file_name || '未在播放')
const desktopLyricNext = computed(() => {
  const next = lyricLines.value[activeLine.value + 1]?.text
  if (next) return next
  return artist.value || album.value || '网盘音乐'
})
const collectTargetTitle = computed(() => stripExt((collectTargetTrack.value || curTrack.value)?.file_name || title.value || '当前歌曲'))
const trackDurationSec = computed(() => normalizeTrackDurationSec(curTrack.value))
const effectiveDur = computed(() => dur.value > 0 ? dur.value : trackDurationSec.value)
const progPct = computed(() => effectiveDur.value ? Math.max(0, Math.min(100, (displayTimeSec.value / effectiveDur.value) * 100)) : 0)
const dispTime = computed(() => fmtTime(displayTimeSec.value))
const isFav = computed(() => curTrack.value ? favSet.value.has(musicTrackKey(curTrack.value)) : false)
const favSet = computed(() => new Set(favs.value.map(musicTrackKey)))
const favKeys = computed(() => favs.value.map(musicTrackKey))
const modeLbl = computed(() => ({ list: '顺序', 'loop-list': '循环', 'loop-one': '单曲', shuffle: '随机' }[mode.value] || ''))
const modeOn = computed(() => mode.value !== 'list')
const currentKey = computed(() => musicTrackKey(curTrack.value))
const shelfCollectionPane = ref<'primary' | 'secondary'>('primary')
const shelfPodcastTracks = computed(() => {
  return playlist.value.filter((track) => {
    const name = `${track.file_name || ''} ${track.description || ''}`.toLowerCase()
    return /播客|podcast|episode|ep\\.?\\s*\\d+|有声|电台|radio|talk|访谈/.test(name) || Number(track.size || 0) > 80 * 1024 * 1024
  })
})
const shelfAlbumCollections = computed(() => {
  const groups = new Map<string, IPageMusicTrack[]>()
  for (const track of playlist.value) {
    const normalized = stripExt(track.file_name || '')
    const parts = normalized.split(/\s+-\s+| - |·/)
    const key = parts.length >= 2 ? parts[0].replace(/^\d+\.\s*/, '').trim() : ''
    if (!key || key.length < 2) continue
    if (!groups.has(key)) groups.set(key, [])
    groups.get(key)!.push(track)
  }
  return Array.from(groups.entries()).filter(([, tracks]) => tracks.length > 1).slice(0, 6)
})
const shelfArtistCollections = computed(() => {
  const groups = new Map<string, IPageMusicTrack[]>()
  for (const track of playlist.value) {
    const normalized = stripExt(track.file_name || '')
    const match = normalized.match(/(?:^|\s)([\u4e00-\u9fa5A-Za-z0-9_]{2,20})\s+-\s+/)
    const key = match?.[1] || ''
    if (!key) continue
    if (!groups.has(key)) groups.set(key, [])
    groups.get(key)!.push(track)
  }
  return Array.from(groups.entries()).filter(([, tracks]) => tracks.length > 1).slice(0, 6)
})
const shelfCards = computed<ShelfCard[]>(() => {
  const makeCollection = (id: string, title: string, subtitle: string, tracks: IPageMusicTrack[]): ShelfCard | null => {
    if (!tracks.length) return null
    const active = id === 'queue' ? curTrack.value : tracks[0]
    return {
      id,
      title,
      subtitle,
      coverUrl: (id === 'queue' ? coverUrl.value : '') || active?.thumbnail || coverUrl.value,
      trackCount: tracks.length,
      data: { source: id, tracks, activeKey: currentKey.value }
    }
  }
  const queueCard = makeCollection('queue', '当前队列', curTrack.value ? `${stripExt(curTrack.value.file_name || '')} · ${playlist.value.length} 首` : `${playlist.value.length} 首`, playlist.value)
  const favoriteCard = visualFx.value.shelfShowFavorites ? makeCollection('favorite', '我的收藏', `${favs.value.length} 首网盘音乐`, favs.value) : null
  const recentCard = visualFx.value.shelfShowRecents ? makeCollection('recent', '最近播放', `${recents.value.length} 首网盘音乐`, recents.value) : null
  const podcastCard = visualFx.value.shelfShowPodcasts ? makeCollection('podcast', '播客队列', `${shelfPodcastTracks.value.length} 集`, shelfPodcastTracks.value) : null
  const localPlaylistCards = localPlaylists.value.map((list) => makeCollection(`local:${list.id}`, list.name, `${list.tracks.length} 首本地歌单`, list.tracks)).filter(Boolean) as ShelfCard[]
  const albumCards = shelfAlbumCollections.value.map(([name, tracks]) => makeCollection(`album:${name}`, name, `${tracks.length} 首专辑集合`, tracks)).filter(Boolean) as ShelfCard[]
  const artistCards = shelfArtistCollections.value.map(([name, tracks]) => makeCollection(`artist:${name}`, name, `${tracks.length} 首艺人集合`, tracks)).filter(Boolean) as ShelfCard[]
  const primaryCollections = [queueCard, ...localPlaylistCards, ...albumCards, ...artistCards].filter(Boolean) as ShelfCard[]
  const secondaryCollections = [favoriteCard, recentCard, podcastCard].filter(Boolean) as ShelfCard[]
  if (visualFx.value.shelfMergeCollections) return [...primaryCollections, ...secondaryCollections]
  if (shelfCollectionPane.value === 'secondary' && secondaryCollections.length) return secondaryCollections.map((card) => ({ ...card, subtitle: `${card.subtitle} · 扩展集合` }))
  return primaryCollections.length ? primaryCollections.map((card) => ({ ...card, subtitle: `${card.subtitle} · 主集合` })) : secondaryCollections
})

function stripExt(n: string) {
  const i = n.lastIndexOf('.')
  return i > 0 ? n.slice(0, i) : n
}
function normalizeTrackDurationSec(track: IPageMusicTrack | undefined | null): number {
  const raw = Number(track?.duration_ms || (track as any)?.duration || 0)
  if (!Number.isFinite(raw) || raw <= 0) return 0
  return raw > 10000 ? Math.round(raw / 1000) : Math.round(raw)
}
function fmtTime(s: number) {
  s = Math.max(0, Math.floor(s))
  const m = Math.floor(s / 60)
  const sec = s % 60
  return `${m}:${sec.toString().padStart(2, '0')}`
}
function loadJson<T>(k: string, fb: T): T {
  try {
    const s = localStorage.getItem(k)
    return s ? JSON.parse(s) ?? fb : fb
  } catch {
    return fb
  }
}
function saveJson(k: string, v: unknown) {
  try { localStorage.setItem(k, JSON.stringify(v)) } catch {}
}
function saveFavs() { saveMusicTrackList('pm.favs', favs.value) }
function saveRecs() { saveMusicTrackList('pm.recs', recents.value) }
function resetVisualFx() {
  visualFx.value = { ...DEFAULT_MUSIC_FX }
}
function setShelfCameraMode(mode: 'static' | 'dynamic') {
  visualFx.value.shelfCameraMode = mode
  visualFx.value.shelfAngleY = mode === 'static' ? -15 : 0
}
function applyFxPreset(config: MusicFxConfig) {
  visualFx.value = { ...config }
}
function defaultVisualFxSlots(): MusicFxUserSlot[] {
  return Array.from({ length: 4 }, (_, index) => ({ id: `slot-${index + 1}`, name: `存档 ${index + 1}`, config: null, updatedAt: 0 }))
}
function saveVisualFxSlots() {
  saveJson('pm.visualFxSlots', visualFxSlots.value)
}
function loadVisualFxSlots() {
  const saved = loadJson<MusicFxUserSlot[]>('pm.visualFxSlots', [])
  const byId = new Map(saved.map((slot) => [slot.id, slot]))
  visualFxSlots.value = defaultVisualFxSlots().map((slot) => {
    const savedSlot = byId.get(slot.id)
    return savedSlot ? { ...slot, ...savedSlot, config: savedSlot.config ? normalizeMusicFxConfig(savedSlot.config) : null } : slot
  })
}
function saveVisualFxSlot(slot: MusicFxUserSlot) {
  const target = visualFxSlots.value.find((item) => item.id === slot.id)
  if (!target) return
  target.config = normalizeMusicFxConfig(visualFx.value)
  target.updatedAt = Date.now()
  saveVisualFxSlots()
  message.success(`已保存 ${target.name}`)
}
function loadVisualFxSlot(slot: MusicFxUserSlot) {
  if (!slot.config) {
    message.warning(`${slot.name} 还没有保存视觉配置`)
    return
  }
  visualFx.value = normalizeMusicFxConfig(slot.config)
}
function exportVisualFx() {
  const blob = new Blob([JSON.stringify(visualFx.value, null, 2)], { type: 'application/json' })
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = 'boxplayer-radio-fx.json'
  a.click()
  URL.revokeObjectURL(a.href)
}
function importVisualFx() {
  const input = document.createElement('input')
  input.type = 'file'
  input.accept = 'application/json,.json'
  input.onchange = async () => {
    const file = input.files?.[0]
    if (!file) return
    const next = parseMusicFxConfigJson(await file.text())
    if (!next) {
      message.error('FX 配置文件无效')
      return
    }
    visualFx.value = next
    message.success('已导入视觉配置')
  }
  input.click()
}

async function sampleCoverPalette() {
  const url = coverUrl.value
  if (!url) {
    message.warning('当前歌曲没有可取色的封面')
    return
  }
  const img = new Image()
  img.crossOrigin = 'anonymous'
  img.decoding = 'async'
  await new Promise<void>((resolve, reject) => {
    img.onload = () => resolve()
    img.onerror = () => reject(new Error('封面加载失败'))
    img.src = url
  }).catch((err) => {
    message.error(err?.message || '封面加载失败')
  })
  if (!img.complete || !img.naturalWidth) return
  const palette = extractCoverPalette(img)
  if (!palette) {
    message.warning('封面取色失败，可能受跨域限制')
    return
  }
  coverPalette.value = palette
  applyCoverPalette(palette.primary)
}

function applyCoverPalette(color: string) {
  visualFx.value.visualTintColor = color
  visualFx.value.uiAccentColor = color
  visualFx.value.shelfAccentColor = color
  visualFx.value.lyricGlowColor = color
  visualFx.value.lyricPrimaryColor = coverPalette.value?.secondary || visualFx.value.lyricPrimaryColor
  visualFx.value.lyricActiveColor = coverPalette.value?.highlight || visualFx.value.lyricActiveColor
}

function scheduleBeatChipHide(delay = 4200) {
  if (beatChipTimer) window.clearTimeout(beatChipTimer)
  beatChipTimer = window.setTimeout(() => {
    beatChipVisible.value = false
    beatChipTimer = null
  }, delay)
}

function beatAnalysisTimeoutMs(mode: 'mr' | 'dj', duration: number) {
  const dur = Number.isFinite(duration) && duration > 0 ? duration : 0
  const base = mode === 'dj' ? 90000 : 75000
  const max = mode === 'dj' ? 150000 : 120000
  return Math.round(Math.min(max, Math.max(base, dur * 320)))
}

async function loadCachedBeatMapForCurrent() {
  const track = curTrack.value
  if (!track || !effectiveDur.value) {
    beatMap.value = null
    return
  }
  const trackKey = musicTrackKey(track)
  const cached = (await getCachedBeatMap(track, 'mr', effectiveDur.value)) || (await getCachedBeatMap(track, 'dj', effectiveDur.value))
  if (!cached || !curTrack.value || musicTrackKey(curTrack.value) !== trackKey) return
  beatMap.value = cached
  beatInfo.value = { mode: cached.mode, bpm: cached.bpm, peaks: cached.peaks, updatedAt: cached.updatedAt }
}

async function analyzeCurrentBeat(mode: 'mr' | 'dj') {
  const a = audioRef.value
  if (!a?.src || beatAnalyzing.value) return
  const controller = new AbortController()
  const timeoutMs = beatAnalysisTimeoutMs(mode, dur.value)
  const timeout = window.setTimeout(() => controller.abort(), timeoutMs)
  beatAnalyzing.value = true
  beatAnalyzingMode.value = mode
  beatChipVisible.value = true
  if (beatChipTimer) {
    window.clearTimeout(beatChipTimer)
    beatChipTimer = null
  }
  try {
    const result = await analyzeAudioElementBeat(a, curTrack.value, mode, dur.value, controller.signal)
    beatInfo.value = { mode, bpm: result.bpm, peaks: result.peaks, updatedAt: result.updatedAt }
    beatMap.value = { mode: result.mode, bpm: result.bpm, peaks: result.peaks, peakTimes: result.peakTimes, duration: result.duration, updatedAt: result.updatedAt }
    message.success(result.bpm ? `节奏分析${result.cacheHit ? '缓存' : ''}完成 ${result.bpm} BPM` : `节奏分析${result.cacheHit ? '缓存' : ''}完成`)
  } catch (err: any) {
    if (err?.name === 'AbortError') message.error(`节奏分析超过 ${Math.round(timeoutMs / 1000)} 秒，已停止`)
    else message.error(err?.message || '节奏分析失败')
  } finally {
    window.clearTimeout(timeout)
    beatAnalyzing.value = false
    beatAnalyzingMode.value = null
    scheduleBeatChipHide()
  }
}

function markControlsVisible(e?: MouseEvent) {
  controlsVisible.value = true
  if (controlsTimer) window.clearTimeout(controlsTimer)
  if (!immersiveMode.value) return
  controlsTimer = window.setTimeout(() => {
    controlsVisible.value = false
    showFxPanel.value = false
    showQueue.value = false
  }, 2200)
}

function toggleImmersiveMode() {
  immersiveMode.value = !immersiveMode.value
  controlsVisible.value = true
  if (!immersiveMode.value && controlsTimer) {
    window.clearTimeout(controlsTimer)
    controlsTimer = null
  } else {
    markControlsVisible()
  }
}

function syncDesktopLyricsWindow() {
  if (!desktopLyricsOn.value) return
  window.WebSendLyric?.({
    text: desktopLyricText.value,
    next: desktopLyricNext.value,
    title: title.value || curTrack.value?.file_name || '',
    artist: artist.value,
    playing: playing.value,
    clickThrough: desktopLyricsClickThrough.value,
    locked: desktopLyricsLocked.value,
    fps: desktopLyricFps.value,
    fontSize: desktopLyricFontSize.value,
    fontWeight: visualFx.value.lyricWeight,
    letterSpacing: visualFx.value.lyricLetterSpacing,
    lineHeight: visualFx.value.lyricLineHeight,
    lyricScale: visualFx.value.lyricSize,
    primaryColor: desktopLyricPrimaryColor.value,
    secondaryColor: desktopLyricSecondaryColor.value,
    opacity: desktopLyricOpacity.value,
    y: desktopLyricY.value,
    backgroundOpacity: desktopLyricBgOpacity.value,
    shake: desktopLyricShake.value,
    fontFamily: desktopLyricFontFamily.value,
    highlightFollow: desktopLyricHighlightFollow.value
  })
}

function toggleDesktopLyrics() {
  desktopLyricsOn.value = !desktopLyricsOn.value
}

function toggleDesktopLyricsClickThrough() {
  desktopLyricsClickThrough.value = !desktopLyricsClickThrough.value
  saveJson('pm.desktopLyricsClickThrough', desktopLyricsClickThrough.value)
  syncDesktopLyricsWindow()
}

function toggleDesktopLyricsLock() {
  desktopLyricsLocked.value = !desktopLyricsLocked.value
  saveJson('pm.desktopLyricsLocked', desktopLyricsLocked.value)
  syncDesktopLyricsWindow()
}

function hotkeyActionForId(id: string) {
  if (id === 'toggle') return 'toggle'
  if (id === 'prev') return 'prev'
  if (id === 'next') return 'next'
  if (id === 'seekBack') return 'seekBack'
  if (id === 'seekForward') return 'seekForward'
  if (id === 'volUp') return 'volUp'
  if (id === 'volDown') return 'volDown'
  if (id === 'immersiveExit') return 'immersiveExit'
  return ''
}

function runHotkeyAction(action: string) {
  if (action === 'toggle') togglePlay()
  else if (action === 'prev') playPrev()
  else if (action === 'next') playNext(false)
  else if (action === 'seekBack') seekRel(-5)
  else if (action === 'seekForward') seekRel(5)
  else if (action === 'volUp') setVol(Math.min(1, vol.value + 0.05))
  else if (action === 'volDown') setVol(Math.max(0, vol.value - 0.05))
  else if (action === 'immersiveExit') {
    immersiveMode.value = false
    controlsVisible.value = true
  } else {
    return false
  }
  return true
}

async function configureGlobalHotkeys() {
  saveJson('pm.globalHotkeysEnabled', globalHotkeysEnabled.value)
  if (!globalHotkeysEnabled.value) {
    await window.WebConfigureGlobalHotkeys?.([])
    globalHotkeyStatus.value = '全局热键已关闭'
    return
  }
  const bindings = hotkeyItems.value.map((item) => ({ action: hotkeyActionForId(item.id), accelerator: item.value, enabled: !!item.value })).filter((item) => item.action && item.accelerator)
  const result = await window.WebConfigureGlobalHotkeys?.(bindings)
  const registered = result?.registered?.length || 0
  const failed = result?.failed?.length || 0
  globalHotkeyStatus.value = failed ? `已注册 ${registered} 个，失败 ${failed} 个` : `已注册 ${registered} 个全局热键`
}

function handleCustomHotkey(e: KeyboardEvent) {
  const key = hotkeyFromEvent(e)
  const item = hotkeyItems.value.find((hotkey) => hotkey.value === key)
  if (!item) return false
  e.preventDefault()
  runHotkeyAction(hotkeyActionForId(item.id))
  return true
}

function captureHotkey(e: KeyboardEvent, item: MineradioHotkey) {
  e.preventDefault()
  item.value = hotkeyFromEvent(e)
  hotkeyCaptureId.value = ''
  saveHotkeys(hotkeyItems.value)
  configureGlobalHotkeys()
}

function resetHotkeys() {
  hotkeyItems.value = DEFAULT_MINERADIO_HOTKEYS.map((item) => ({ ...item }))
  saveHotkeys(hotkeyItems.value)
  configureGlobalHotkeys()
}

function reloadLocalPlaylists() {
  localPlaylists.value = loadPlaylists()
}

function openCollectModal(track: IPageMusicTrack | undefined | null = curTrack.value) {
  collectTargetTrack.value = track ? { ...track } : null
  showCollectModal.value = true
}

function collectCurrentToPlaylist(list: LocalPlaylist) {
  const track = collectTargetTrack.value || curTrack.value
  if (!track) return
  const next = localPlaylists.value.map((item) => item.id === list.id ? addTracksToList(item, [track]) : item)
  localPlaylists.value = next
  savePlaylists(next)
  message.success('已收藏到歌单')
}

function collectCurrentToNewPlaylist() {
  const track = collectTargetTrack.value || curTrack.value
  if (!track) return
  const name = collectNewPlaylistName.value.trim() || `歌单 ${localPlaylists.value.length + 1}`
  const next = [createPlaylist(name, [track]), ...localPlaylists.value]
  localPlaylists.value = next
  collectNewPlaylistName.value = ''
  savePlaylists(next)
  message.success('已新建歌单并收藏')
}

function toggleGestureControl() {
  visualFx.value.gestureControlEnabled = !visualFx.value.gestureControlEnabled
  gestureHudVisible.value = visualFx.value.gestureControlEnabled
  gestureHudText.value = visualFx.value.gestureControlEnabled ? '手势控制已开启：当前以鼠标/滚轮模拟 BoxPlayer Radio 手势反馈。' : '手势控制已关闭'
  if (!visualFx.value.gestureControlEnabled) window.setTimeout(() => gestureHudVisible.value = false, 1100)
}

function customLyricStorageKey(track: IPageMusicTrack | undefined | null = curTrack.value) {
  return track ? `pm.customLyric.${musicTrackKey(track)}` : ''
}
function lyricSyncStorageKey(track: IPageMusicTrack | undefined | null = curTrack.value) {
  return track ? `pm.lyricTimeOffset.${musicTrackKey(track)}` : ''
}
function customCoverStorageKey(track: IPageMusicTrack | undefined | null = curTrack.value) {
  return track ? `pm.customCover.${musicTrackKey(track)}` : ''
}
function customBackgroundStorageKey() {
  return 'pm.customBackground'
}

function loadLyricTimeOffsetForTrack(track: IPageMusicTrack | undefined | null = curTrack.value) {
  const key = lyricSyncStorageKey(track)
  lyricTimeOffsetSec.value = key ? loadJson(key, 0) : 0
}

function saveLyricTimeOffset() {
  const key = lyricSyncStorageKey()
  if (!key) return
  saveJson(key, Number(lyricTimeOffsetSec.value.toFixed(2)))
}

function adjustLyricTimeOffset(delta: number) {
  lyricTimeOffsetSec.value = Math.max(-8, Math.min(8, Number((lyricTimeOffsetSec.value + delta).toFixed(2))))
  saveLyricTimeOffset()
}

function resetLyricTimeOffset() {
  lyricTimeOffsetSec.value = 0
  saveLyricTimeOffset()
}

async function applyCustomLyric(text: string, persist = true) {
  const track = curTrack.value
  if (!track) return
  const lines = parseLrc(text, dur.value || undefined)
  const lyricLineSource = /\[\d{1,2}:\d{1,2}(?:[\.:]\d{1,3})?\]|\[\d{1,8}(?:,\d{1,8})?\]/.test(text) ? 'timed' : lines.length ? 'plain' : 'empty'
  const key = customLyricStorageKey(track)
  customLyricAppliedKey = key
  meta.value = {
    ...(meta.value || {
      title: stripExt(track.file_name || ''),
      artist: '',
      album: '',
      cover: '',
      lrc: '',
      lines: []
    }),
    lrc: text,
    lines,
    metadataSources: Array.from(new Set([...(meta.value?.metadataSources || ['filename']), 'custom:lyrics'])),
    debug: {
      query: {
        filename: track.file_name,
        artist: meta.value?.artist || '',
        title: meta.value?.title || stripExt(track.file_name || ''),
        album: meta.value?.album || '',
        durationSec: dur.value || undefined
      },
      lyricProvider: 'musicsdk',
      lrcLength: text.length,
      parsedLineCount: lines.length,
      lyricLineSource,
      reason: lines.length ? `已使用自定义歌词 ${lines.length} 行` : '自定义歌词未解析出可展示行'
    }
  }
  if (persist && key) await setMineradioValue(key, { text, updatedAt: Date.now(), source: 'custom' })
  showLyrics.value = true
}

async function deleteCustomLyric() {
  const key = customLyricStorageKey()
  if (key) {
    localStorage.removeItem(key)
    await deleteMineradioValue(key)
  }
  customLyricAppliedKey = ''
  lastMetaQueryKey = ''
  loadMeta()
}

async function loadCustomLyricForTrack(track: IPageMusicTrack | undefined | null = curTrack.value) {
  const key = customLyricStorageKey(track)
  if (!key) return false
  const stored = await getMineradioValue<{ text: string }>(key).catch(() => null)
  let text = stored?.text || ''
  const legacy = localStorage.getItem(key)
  if (!text && legacy) {
    text = legacy
    await setMineradioValue(key, { text, updatedAt: Date.now(), source: 'legacy-localStorage' }).catch(() => {})
    localStorage.removeItem(key)
  }
  if (!text) return false
  await applyCustomLyric(text, false)
  return true
}

async function loadCustomCoverForTrack(track: IPageMusicTrack | undefined | null = curTrack.value) {
  const key = customCoverStorageKey(track)
  if (!key) return
  const stored = await getMineradioValue<{ blob: Blob }>(key).catch(() => null)
  if (!stored?.blob) return
  if (customCoverUrl.value) URL.revokeObjectURL(customCoverUrl.value)
  customCoverUrl.value = URL.createObjectURL(stored.blob)
}

async function commitCoverCrop(blob: Blob) {
  if (customCoverUrl.value) URL.revokeObjectURL(customCoverUrl.value)
  customCoverUrl.value = URL.createObjectURL(blob)
  const key = customCoverStorageKey()
  if (key) await setMineradioValue(key, { blob, updatedAt: Date.now(), source: 'cover-crop' })
}

async function clearCustomCover() {
  const key = customCoverStorageKey()
  if (key) await deleteMineradioValue(key)
  if (customCoverUrl.value) URL.revokeObjectURL(customCoverUrl.value)
  customCoverUrl.value = ''
}

async function loadCustomBackground() {
  const stored = await getMineradioValue<{ blob: Blob; type?: 'image' | 'video' }>(customBackgroundStorageKey()).catch(() => null)
  if (!stored?.blob) return
  if (customBackgroundUrl.value) URL.revokeObjectURL(customBackgroundUrl.value)
  customBackgroundType.value = stored.type || (stored.blob.type.startsWith('video/') ? 'video' : 'image')
  customBackgroundUrl.value = URL.createObjectURL(stored.blob)
}

function pickCustomBackground() {
  const input = document.createElement('input')
  input.type = 'file'
  input.accept = 'image/*,video/*'
  input.onchange = async () => {
    const file = input.files?.[0]
    if (!file) return
    if (!file.type.startsWith('image/') && !file.type.startsWith('video/')) {
      message.error('请选择图片或视频背景')
      return
    }
    if (customBackgroundUrl.value) URL.revokeObjectURL(customBackgroundUrl.value)
    const type = file.type.startsWith('video/') ? 'video' : 'image'
    customBackgroundType.value = type
    customBackgroundUrl.value = URL.createObjectURL(file)
    await setMineradioValue(customBackgroundStorageKey(), { blob: file, type, updatedAt: Date.now(), source: 'background-media' })
    message.success('已应用自定义背景')
  }
  input.click()
}

async function clearCustomBackground() {
  await deleteMineradioValue(customBackgroundStorageKey())
  if (customBackgroundUrl.value) URL.revokeObjectURL(customBackgroundUrl.value)
  customBackgroundUrl.value = ''
  customBackgroundType.value = 'image'
}
function formatLyricDebugTitle(data: MusicMetadata | null) {
  const debug = data?.debug
  if (!debug) return ''
  const lines = [
    `歌词接口: musicsdk (网易云 wy -> 酷狗 kg)`,
    `元数据来源: ${(data?.metadataSources || []).join(', ') || 'filename'}`,
    `查询: ${debug.query.artist ? `${debug.query.artist} - ` : ''}${debug.query.title}`,
    `文件: ${debug.query.filename}`,
    `结果: ${debug.reason}`,
    `LRC字符: ${debug.lrcLength}`,
    `解析行数: ${debug.parsedLineCount}`
  ]
  if (debug.lyric?.matched) lines.push(`匹配歌曲: ${debug.lyric.matched.source} ${debug.lyric.matched.name} / ${debug.lyric.matched.singer}`)
  for (const step of debug.lyric?.steps || []) lines.push(`${step.source}: ${step.status} ${step.message}${step.lyricLength !== undefined ? ` lyric=${step.lyricLength}` : ''}`)
  return lines.join('\n')
}

function toggleFav(track: IPageMusicTrack | undefined | null = curTrack.value) {
  if (!track) return
  const k = musicTrackKey(track)
  const i = favs.value.findIndex((f) => musicTrackKey(f) === k)
  if (i >= 0) favs.value.splice(i, 1)
  else favs.value.unshift({ ...track })
  saveFavs()
}
function removeFav(t: IPageMusicTrack) {
  favs.value = favs.value.filter((f) => musicTrackKey(f) !== musicTrackKey(t))
  saveFavs()
}
function pushRec(t: IPageMusicTrack | undefined | null) {
  if (!t) return
  const k = musicTrackKey(t)
  const i = recents.value.findIndex((r) => musicTrackKey(r) === k)
  if (i >= 0) recents.value.splice(i, 1)
  recents.value.unshift({ ...t })
  if (recents.value.length > 80) recents.value.length = 80
  saveRecs()
}

async function resolveUrl(idx: number) {
  const t = playlist.value[idx]
  if (!t) return ''
  if (t.local_url) return t.local_url
  const d = await getRawUrl(t.user_id, t.drive_id, t.file_id, t.encType || '', t.password || '', false, 'audio')
  if (typeof d === 'string') throw new Error(d || '获取地址失败')
  return d.url || ''
}

async function loadIdx(idx: number, auto = true) {
  if (!playlist.value.length) return
  idx = Math.max(0, Math.min(idx, playlist.value.length - 1))
  curIdx.value = idx
  loadLyricTimeOffsetForTrack(playlist.value[idx])
  errMsg.value = ''
  dur.value = 0
  curTime.value = 0
  beatMap.value = null
  beatInfo.value = null
  beatChipVisible.value = false
  lastMetaQueryKey = ''
  customLyricAppliedKey = ''
  if (customCoverUrl.value) URL.revokeObjectURL(customCoverUrl.value)
  customCoverUrl.value = ''
  const a = audioRef.value
  if (!a) return
  try {
    loading.value = true
    const url = await resolveUrl(idx)
    if (!url) throw new Error('未获取到播放地址')
    a.src = url
    a.load()
    if (auto) {
      try {
        await a.play()
        playing.value = true
      } catch {
        playing.value = false
      }
    }
  } catch (e: any) {
    errMsg.value = e?.message || String(e)
    message.error('加载失败: ' + errMsg.value)
    playing.value = false
  } finally {
    loading.value = false
  }
  await loadCustomCoverForTrack(curTrack.value)
  if (!(await loadCustomLyricForTrack())) loadMeta()
  pushRec(curTrack.value)
}

async function loadMeta() {
  const t = curTrack.value
  metaTok++
  const tok = metaTok
  if (!t) {
    meta.value = null
    return
  }
  const duration = Number.isFinite(effectiveDur.value) && effectiveDur.value > 0 ? Math.round(effectiveDur.value) : 0
  const queryKey = `${musicTrackKey(t)}|${duration}`
  if (queryKey === lastMetaQueryKey && meta.value) return
  lastMetaQueryKey = queryKey
  metaLoad.value = true
  try {
    const d = await fetchMusicMetadata({ filename: t.file_name, durationSec: duration || undefined })
    if (tok !== metaTok) return
    meta.value = d
  } catch {
    if (tok === metaTok) meta.value = null
  } finally {
    if (tok === metaTok) metaLoad.value = false
  }
}

function togglePlay() {
  const a = audioRef.value
  if (!a) return
  if (!a.src) return loadIdx(curIdx.value, true)
  if (a.paused) a.play().then(() => playing.value = true).catch(() => playing.value = false)
  else {
    a.pause()
    playing.value = false
  }
}
function playPrev() {
  if (!playlist.value.length) return
  if (mode.value === 'shuffle') {
    if (shuffleHist.value.length > 1) {
      shuffleHist.value.pop()
      return loadIdx(shuffleHist.value[shuffleHist.value.length - 1], true)
    }
    return loadIdx(pickShuffle(), true)
  }
  loadIdx(curIdx.value <= 0 ? playlist.value.length - 1 : curIdx.value - 1, true)
}
function playNext(auto = false) {
  if (!playlist.value.length) return
  if (mode.value === 'loop-one' && auto) return loadIdx(curIdx.value, true)
  if (mode.value === 'shuffle') return loadIdx(pickShuffle(), true)
  let n = curIdx.value + 1
  if (n >= playlist.value.length) {
    if (mode.value === 'list' && auto) {
      playing.value = false
      return
    }
    n = 0
  }
  loadIdx(n, true)
}
function pickShuffle() {
  if (playlist.value.length <= 1) return 0
  let n = curIdx.value
  let tries = 0
  while (n === curIdx.value && tries++ < 32) n = Math.floor(Math.random() * playlist.value.length)
  shuffleHist.value.push(n)
  if (shuffleHist.value.length > 100) shuffleHist.value.shift()
  return n
}
function playFrom(t: IPageMusicTrack) {
  const k = musicTrackKey(t)
  let i = playlist.value.findIndex((p) => musicTrackKey(p) === k)
  if (i < 0) {
    playlist.value.push({ ...t })
    i = playlist.value.length - 1
  }
  loadIdx(i, true)
}
function removeQueueAt(index: number) {
  if (index < 0 || index >= playlist.value.length) return
  const wasCurrent = index === curIdx.value
  playlist.value.splice(index, 1)
  if (!playlist.value.length) {
    curIdx.value = 0
    curTime.value = 0
    dur.value = 0
    playing.value = false
    showQueue.value = false
    try {
      if (audioRef.value) {
        audioRef.value.pause()
        audioRef.value.removeAttribute('src')
        audioRef.value.load()
      }
    } catch {}
    return
  }
  if (index < curIdx.value) curIdx.value -= 1
  else if (wasCurrent) loadIdx(Math.min(curIdx.value, playlist.value.length - 1), true)
}
function moveQueueNext(index: number) {
  if (index < 0 || index >= playlist.value.length) return
  if (index === curIdx.value || index === curIdx.value + 1) return
  const [track] = playlist.value.splice(index, 1)
  const nextIndex = Math.min(curIdx.value + 1, playlist.value.length)
  playlist.value.splice(nextIndex, 0, track)
  if (index < curIdx.value) curIdx.value -= 1
}
function playShelfCard(card: ShelfCard) {
  const track = card.data?.track || card.data?.tracks?.[0] || card.data
  playShelfTrack(track)
}
function playShelfTrack(track: IPageMusicTrack | undefined | null) {
  if (!track) return
  const key = musicTrackKey(track)
  const index = playlist.value.findIndex((item) => musicTrackKey(item) === key)
  if (index >= 0) loadIdx(index, true)
  else playFrom(track)
}
function queueShelfTrackNext(track: IPageMusicTrack | undefined | null) {
  if (!track) return
  const key = musicTrackKey(track)
  let index = playlist.value.findIndex((item) => musicTrackKey(item) === key)
  if (index < 0) {
    playlist.value.push({ ...track })
    index = playlist.value.length - 1
  }
  moveQueueNext(index)
}
function collectShelfTrack(track: IPageMusicTrack | undefined | null) {
  if (!track) return
  openCollectModal(track)
}
function handleShelfBoundaryScroll(direction: -1 | 1) {
  if (visualFx.value.shelfMergeCollections) return false
  if (direction > 0 && shelfCollectionPane.value === 'primary') {
    shelfCollectionPane.value = 'secondary'
    return true
  }
  if (direction < 0 && shelfCollectionPane.value === 'secondary') {
    shelfCollectionPane.value = 'primary'
    return true
  }
  return false
}
function seekRel(d: number) {
  const a = audioRef.value
  if (a && dur.value) {
    const next = Math.max(0, Math.min(dur.value, (a.currentTime || 0) + d))
    a.currentTime = next
    curTime.value = next
    syncAudioClock()
  }
}
function seekTo(time: number) {
  const next = Math.max(0, Math.min(effectiveDur.value || time, time - lyricTimeOffsetSec.value))
  if (audioRef.value) audioRef.value.currentTime = next
  curTime.value = next
  syncAudioClock()
}
function setVol(v: number) {
  vol.value = Math.max(0, Math.min(1, v))
  if (audioRef.value) {
    audioRef.value.volume = vol.value
    if (vol.value > 0 && muted.value) {
      muted.value = false
      audioRef.value.muted = false
    }
  }
  saveJson('pm.vol', vol.value)
}
function toggleMute() {
  if (!audioRef.value) return
  muted.value = !muted.value
  audioRef.value.muted = muted.value
}
function cycleMode() {
  const o: PlayMode[] = ['loop-list', 'loop-one', 'shuffle', 'list']
  mode.value = o[(o.indexOf(mode.value) + 1) % o.length]
  shuffleHist.value = []
  saveJson('pm.mode', mode.value)
}
function toggleLyrics() {
  showLyrics.value = !showLyrics.value
  nextTick(() => {
    const msg = `[PageMusic] lyrics toggle show=${showLyrics.value} sidePanel=${!!props.sidePanel} hasLyrics=${hasLyrics.value} lines=${lyricLines.value.length}`
    console.info(msg)
  })
}
function onProgDown(e: MouseEvent) {
  const bar = e.currentTarget as HTMLElement
  if (!bar || !effectiveDur.value) return
  const rect = bar.getBoundingClientRect()
  const upd = (cx: number) => seekPreview.value = (Math.max(0, Math.min(rect.width, cx - rect.left)) / rect.width) * effectiveDur.value
  upd(e.clientX)
  const mv = (ev: MouseEvent) => upd(ev.clientX)
  const up = (ev: MouseEvent) => {
    upd(ev.clientX)
    window.removeEventListener('mousemove', mv)
    window.removeEventListener('mouseup', up)
    if (audioRef.value && seekPreview.value != null) {
      audioRef.value.currentTime = seekPreview.value
      curTime.value = seekPreview.value
      syncAudioClock()
    }
    seekPreview.value = null
  }
  window.addEventListener('mousemove', mv)
  window.addEventListener('mouseup', up)
}
function onVolDown(e: MouseEvent) {
  const bar = e.currentTarget as HTMLElement
  if (!bar) return
  const rect = bar.getBoundingClientRect()
  const upd = (cx: number) => setVol(Math.max(0, Math.min(rect.width, cx - rect.left)) / rect.width)
  upd(e.clientX)
  const mv = (ev: MouseEvent) => upd(ev.clientX)
  const up = () => {
    window.removeEventListener('mousemove', mv)
    window.removeEventListener('mouseup', up)
  }
  window.addEventListener('mousemove', mv)
  window.addEventListener('mouseup', up)
}
function handleHide() {
  try { audioRef.value?.pause() } catch {}
  window.WebToWindow?.({ cmd: 'close' })
  if (!window.WebToWindow) window.close()
}
function handleMin() { window.WebToWindow?.({ cmd: 'minsize' }) }
function handleMax() { window.WebToWindow?.({ cmd: 'maxsize' }) }
function handleRootDblClick(e: MouseEvent) {
  if (shouldHandleMusicPlayerDblClick(e)) handleMax()
}
function syncAudioClock() {
  const a = audioRef.value
  if (!a) return
  const nextTime = Number.isFinite(a.currentTime) ? a.currentTime : 0
  const nextDur = Number.isFinite(a.duration) ? a.duration : 0
  if (Math.abs(curTime.value - nextTime) > 0.015) curTime.value = nextTime
  if (Math.abs(dur.value - nextDur) > 0.05) dur.value = nextDur
}
function startClockLoop() {
  if (clockRaf) return
  const tick = () => {
    syncAudioClock()
    clockRaf = requestAnimationFrame(tick)
  }
  clockRaf = requestAnimationFrame(tick)
}
function stopClockLoop() {
  if (!clockRaf) return
  cancelAnimationFrame(clockRaf)
  clockRaf = 0
}
function onLoaded() {
  syncAudioClock()
  if (effectiveDur.value > 0 && customLyricAppliedKey !== customLyricStorageKey()) loadMeta()
  loadCachedBeatMapForCurrent()
  if (playing.value || !audioRef.value?.paused) startClockLoop()
  fadeTo(1, 460)
}
function onTime() { syncAudioClock() }
function onEnded() {
  stopClockLoop()
  syncAudioClock()
  fadeTo(0.0001, 420)
  playNext(true)
}
function onErr() {
  if (!audioRef.value?.src) return
  errMsg.value = '播放出错'
  playing.value = false
}
function onPlay() { playing.value = true; syncAudioClock(); startClockLoop() }
function onPause() { playing.value = false; syncAudioClock(); stopClockLoop() }
function emitState() {
  emit('state-change', {
    title: title.value || curTrack.value?.file_name || '',
    artist: artist.value,
    album: album.value,
    coverUrl: coverUrl.value,
    isPlaying: playing.value,
    isLoading: loading.value,
    currentTime: displayTimeSec.value,
    duration: dur.value,
    progressPercent: progPct.value,
    hasTrack: !!curTrack.value
  })
}

function loadPageMusic(d: any) {
  playlist.value = (d.playlist || []).slice()
  if (!playlist.value.length) playlist.value = [{ user_id: d.user_id, drive_id: d.drive_id, file_id: d.file_id, parent_file_id: d.parent_file_id, file_name: d.file_name, encType: d.encType, password: d.password }]
  let idx = playlist.value.findIndex((t) => t.file_id === d.file_id)
  if (idx < 0) idx = 0
  curIdx.value = idx
  loadIdx(idx, true)
}

onMounted(() => {
  window.addEventListener('keydown', onKeyDown, true)
  vol.value = loadJson('pm.vol', 0.8)
  mode.value = (loadJson('pm.mode', 'loop-list') as PlayMode) || 'loop-list'
  showLyrics.value = loadJson('pm.lyrics', false)
  visualFx.value = normalizeMusicFxConfig(loadJson('pm.visualFx', DEFAULT_MUSIC_FX))
  hotkeyItems.value = loadHotkeys()
  loadVisualFxSlots()
  desktopLyricsOn.value = loadJson('pm.desktopLyricsOn', false)
  desktopLyricsClickThrough.value = loadJson('pm.desktopLyricsClickThrough', false)
  desktopLyricsLocked.value = loadJson('pm.desktopLyricsLocked', false)
  desktopLyricFps.value = loadJson('pm.desktopLyricFps', 60)
  desktopLyricFontSize.value = loadJson('pm.desktopLyricFontSize', 40)
  desktopLyricPrimaryColor.value = loadJson('pm.desktopLyricPrimaryColor', '#fff9e8')
  desktopLyricSecondaryColor.value = loadJson('pm.desktopLyricSecondaryColor', '#d3e5ec')
  desktopLyricOpacity.value = loadJson('pm.desktopLyricOpacity', 1)
  desktopLyricY.value = loadJson('pm.desktopLyricY', 0.5)
  desktopLyricBgOpacity.value = loadJson('pm.desktopLyricBgOpacity', 0.42)
  desktopLyricShake.value = loadJson('pm.desktopLyricShake', 0.16)
  desktopLyricFontFamily.value = loadJson('pm.desktopLyricFontFamily', 'system')
  desktopLyricHighlightFollow.value = loadJson('pm.desktopLyricHighlightFollow', true)
  if (desktopLyricsOn.value) {
    window.WebOpenLyric?.()
    setTimeout(syncDesktopLyricsWindow, 180)
  }
  diySimpleMode.value = loadJson('pm.diySimpleMode', false)
  globalHotkeysEnabled.value = loadJson('pm.globalHotkeysEnabled', false)
  reloadLocalPlaylists()
  offGlobalHotkey = window.WebOnGlobalHotkey?.((data: any) => runHotkeyAction(String(data?.action || ''))) || null
  if (globalHotkeysEnabled.value) configureGlobalHotkeys()
  const tab = loadJson('pm.tab', 'queue')
  if (['queue', 'fav', 'recent'].includes(String(tab))) panelTab.value = tab as PanelTab
  favs.value = loadMusicTrackList('pm.favs', { legacyKeys: ['pageMusic.favorites'] })
  recents.value = loadMusicTrackList('pm.recs')
  loadCustomBackground()
  if (audioRef.value) {
    audioRef.value.volume = vol.value
    audioRef.value.crossOrigin = 'anonymous'
    bindAudio(audioRef.value)
    initAudioEngine()
  }
  if (props.sidePanel) return
  if (visualFx.value.splashEnabled) {
    showSplash.value = true
    window.setTimeout(() => showSplash.value = false, 2100)
  }
  const d = appStore.pageMusic
  if (!d) {
    message.error('未提供音乐播放参数')
    return
  }
  document.title = d.file_name || '音乐播放器'
  loadPageMusic(d)
})

watch(curTrack, (t) => { if (t) document.title = t.file_name })
watch([curTrack, playing, loading, curTime, dur, coverUrl, artist, album, progPct, displayTimeSec], emitState, { immediate: true })
watch(() => musicPlayerStore.commandSeq, () => {
  if (!props.sidePanel) return
  if (musicPlayerStore.command === 'toggle') togglePlay()
  else if (musicPlayerStore.command === 'prev') playPrev()
  else if (musicPlayerStore.command === 'next') playNext(false)
})
watch(() => musicPlayerStore.loadSeq, () => {
  if (!props.sidePanel) return
  const d = musicPlayerStore.pendingLoad
  if (!d) return
  loadPageMusic(d)
})
watch(showLyrics, (v) => saveJson('pm.lyrics', v))
watch(panelTab, (v) => {
  saveJson('pm.tab', v)
  listFilter.value = ''
})
watch(visualFx, (v) => saveJson('pm.visualFx', v), { deep: true })
watch(desktopLyricsOn, (v) => {
  saveJson('pm.desktopLyricsOn', v)
  if (v) {
    window.WebOpenLyric?.()
    setTimeout(syncDesktopLyricsWindow, 120)
  } else {
    window.WebCloseLyric?.()
  }
})
watch([desktopLyricText, desktopLyricNext, title, artist, playing, desktopLyricFontSize, desktopLyricPrimaryColor, desktopLyricSecondaryColor, desktopLyricOpacity, desktopLyricY, desktopLyricBgOpacity, desktopLyricShake, desktopLyricFontFamily, desktopLyricHighlightFollow, visualFx], syncDesktopLyricsWindow, { deep: true })
watch([desktopLyricsClickThrough, desktopLyricsLocked, desktopLyricFps], syncDesktopLyricsWindow)
watch(desktopLyricFontSize, (v) => saveJson('pm.desktopLyricFontSize', v))
watch(desktopLyricFps, (v) => saveJson('pm.desktopLyricFps', v))
watch(desktopLyricPrimaryColor, (v) => saveJson('pm.desktopLyricPrimaryColor', v))
watch(desktopLyricSecondaryColor, (v) => saveJson('pm.desktopLyricSecondaryColor', v))
watch(desktopLyricOpacity, (v) => saveJson('pm.desktopLyricOpacity', v))
watch(desktopLyricY, (v) => saveJson('pm.desktopLyricY', v))
watch(desktopLyricBgOpacity, (v) => saveJson('pm.desktopLyricBgOpacity', v))
watch(desktopLyricShake, (v) => saveJson('pm.desktopLyricShake', v))
watch(desktopLyricFontFamily, (v) => saveJson('pm.desktopLyricFontFamily', v))
watch(desktopLyricHighlightFollow, (v) => saveJson('pm.desktopLyricHighlightFollow', v))
watch(diySimpleMode, (v) => saveJson('pm.diySimpleMode', v))
watch(() => visualFx.value.gestureControlEnabled, (v) => {
  gestureHudVisible.value = !!v
  if (v) gestureHudText.value = '手势 HUD：掌心推开/捏合旋转以鼠标和滚轮模拟，摄像头源未接入外部服务。'
})
watch(() => visualFx.value.liveBackgroundKeep, () => {
  if (visualFx.value.liveBackgroundKeep && visualFx.value.renderQuality === 'release') visualFx.value.renderQuality = 'keep'
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown, true)
  stopClockLoop()
  if (beatChipTimer) window.clearTimeout(beatChipTimer)
  if (offGlobalHotkey) offGlobalHotkey()
  window.WebConfigureGlobalHotkeys?.([])
  try { audioRef.value?.pause() } catch {}
  if (customCoverUrl.value) URL.revokeObjectURL(customCoverUrl.value)
  if (customBackgroundUrl.value) URL.revokeObjectURL(customBackgroundUrl.value)
  if (desktopLyricsOn.value) window.WebCloseLyric?.()
})

defineExpose({ togglePlay, playPrev, playNext, seekRel })
</script>

<template>
  <div
    :class="['mineradio-player', props.embedded ? 'embedded' : '', props.sidePanel ? 'side-panel' : '', immersiveMode ? 'immersive' : '', diySimpleMode ? 'simple-mode' : '', controlsVisible ? 'controls-visible' : 'controls-hidden']"
    :style="{ '--mineradio-glass-chromatic': String(visualFx.glassChromaticOffset) }"
    @dblclick="handleRootDblClick"
    @mousemove="markControlsVisible"
  >
    <svg id="control-glass-svg" class="control-glass-filter-svg" xmlns="http://www.w3.org/2000/svg" aria-hidden="true" focusable="false">
      <defs>
        <filter id="mineradio-control-glass-filter" color-interpolation-filters="sRGB" x="-12%" y="-28%" width="124%" height="156%">
          <feImage id="control-glass-map" x="0" y="0" width="100%" height="100%" preserveAspectRatio="none" result="map"></feImage>
          <feDisplacementMap in="SourceGraphic" in2="map" :scale="180 * visualFx.glassChromaticOffset" xChannelSelector="R" yChannelSelector="B" result="dispRed"></feDisplacementMap>
          <feOffset in="dispRed" dx="-90" dy="0" result="dispRedShifted"></feOffset>
          <feMerge result="dispRedAligned"><feMergeNode in="SourceGraphic"></feMergeNode><feMergeNode in="dispRedShifted"></feMergeNode></feMerge>
          <feColorMatrix in="dispRedAligned" type="matrix" values="1 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 1 0" result="red"></feColorMatrix>
          <feDisplacementMap in="SourceGraphic" in2="map" :scale="170 * visualFx.glassChromaticOffset" xChannelSelector="R" yChannelSelector="B" result="dispGreen"></feDisplacementMap>
          <feOffset in="dispGreen" dx="-90" dy="0" result="dispGreenShifted"></feOffset>
          <feMerge result="dispGreenAligned"><feMergeNode in="SourceGraphic"></feMergeNode><feMergeNode in="dispGreenShifted"></feMergeNode></feMerge>
          <feColorMatrix in="dispGreenAligned" type="matrix" values="0 0 0 0 0  0 1 0 0 0  0 0 0 0 0  0 0 0 1 0" result="green"></feColorMatrix>
          <feDisplacementMap in="SourceGraphic" in2="map" :scale="160 * visualFx.glassChromaticOffset" xChannelSelector="R" yChannelSelector="B" result="dispBlue"></feDisplacementMap>
          <feOffset in="dispBlue" dx="-90" dy="0" result="dispBlueShifted"></feOffset>
          <feMerge result="dispBlueAligned"><feMergeNode in="SourceGraphic"></feMergeNode><feMergeNode in="dispBlueShifted"></feMergeNode></feMerge>
          <feColorMatrix in="dispBlueAligned" type="matrix" values="0 0 0 0 0  0 0 0 0 0  0 0 1 0 0  0 0 0 1 0" result="blue"></feColorMatrix>
          <feBlend in="red" in2="green" mode="screen" result="rg"></feBlend>
          <feBlend in="rg" in2="blue" mode="screen" result="output"></feBlend>
          <feGaussianBlur in="output" stdDeviation="0.5"></feGaussianBlur>
        </filter>
      </defs>
    </svg>

    <div v-if="!props.embedded" class="mineradio-titlebar q-electron-drag">
      <button class="win-dot close" @click.stop="handleHide"></button>
      <button class="win-dot min" @click.stop="handleMin"></button>
      <button class="win-dot max" @click.stop="handleMax"></button>
    </div>

    <MusicImmersiveStage
      :active-line="activeLine"
      :artist="artist"
      :cover-url="coverUrl"
      :current-time="lyricClockSec"
      :has-lyrics="hasLyrics"
      :lyric-debug-text="lyricDebugText"
      :lyric-debug-title="lyricDebugTitle"
      :lyric-lines="lyricLines"
      :meta-load="metaLoad"
      :audio-el="audioRef"
      :beat-map="beatMap"
      :playing="playing"
      :show-lyrics="showLyrics"
      :title="title"
      :visual-fx="visualFx"
      :custom-background-type="customBackgroundType"
      :custom-background-url="customBackgroundUrl"
      :shelf-cards="shelfCards"
      :on-shelf-boundary-scroll="handleShelfBoundaryScroll"
      @seek-lyric="seekTo"
      @shelf-card-click="playShelfCard"
      @shelf-track-collect="collectShelfTrack"
      @shelf-track-click="playShelfTrack"
      @shelf-track-next="queueShelfTrackNext"
    />

    <div v-if="desktopLyricsOn && curTrack" :class="['desktop-lyric-preview', playing ? 'visible' : '']" @dblclick.stop="showTrackDetail = true">
      <div class="desktop-lyric-line active" :title="desktopLyricText">{{ desktopLyricText }}</div>
      <div class="desktop-lyric-line next" :title="desktopLyricNext">{{ desktopLyricNext }}</div>
      <div class="desktop-lyric-tools">
        <button type="button" :title="desktopLyricsClickThrough ? '取消点击穿透' : '启用点击穿透'" @click.stop="toggleDesktopLyricsClickThrough">{{ desktopLyricsClickThrough ? '解锁' : '穿透' }}</button>
        <button type="button" :title="desktopLyricsLocked ? '解除桌面歌词锁定' : '锁定桌面歌词窗口'" @click.stop="toggleDesktopLyricsLock">{{ desktopLyricsLocked ? '锁定' : '可拖' }}</button>
        <button type="button" title="歌曲详情" @click.stop="showTrackDetail = true">详情</button>
        <button type="button" title="关闭桌面歌词" @click.stop="desktopLyricsOn = false">关闭</button>
      </div>
    </div>

    <div v-if="showSplash" class="mineradio-splash">
      <div class="mineradio-splash-noise"></div>
      <div class="mineradio-splash-wordmark">
        <span class="splash-word-mine">BoxPlayer</span>
        <span class="splash-word-radio">Radio</span>
      </div>
      <div class="mineradio-splash-line"></div>
      <div class="mineradio-splash-enter">CLICK / ENTER</div>
    </div>
    <div v-if="beatChipVisible && (beatAnalyzing || beatInfo)" class="music-beat-chip">
      <span>{{ beatAnalyzing ? '分析中' : `${beatInfo?.bpm || '--'} BPM` }}</span>
      <small>{{ (beatAnalyzing ? beatAnalyzingMode : beatInfo?.mode) === 'dj' ? 'DJ Beat' : 'BP Beat' }}</small>
    </div>
    <div v-if="gestureHudVisible" class="gesture-hud show">
      <b>GESTURE HUD</b>
      <span>{{ gestureHudText }}</span>
    </div>

    <div :class="['music-fx-panel', showFxPanel ? 'show' : '']" @dblclick.stop>
      <div class="music-fx-head">
        <div>
          <div class="music-fx-title">视觉控制台</div>
          <div class="music-fx-sub">BOXPLAYER RADIO VISUALS</div>
        </div>
        <button class="music-fx-reset" title="恢复默认" @click="resetVisualFx">
          <RotateCcw :size="16" :stroke-width="1.9" />
        </button>
      </div>
      <div class="music-fx-presets">
        <button v-for="preset in MUSIC_FX_PRESETS" :key="preset.id" class="music-fx-preset" @click="applyFxPreset(preset.config)">{{ preset.name }}</button>
      </div>
      <div class="music-fx-user-slots">
        <div v-for="slot in visualFxSlots" :key="slot.id" class="music-fx-slot">
          <button type="button" @click="loadVisualFxSlot(slot)">
            <span>{{ slot.name }}</span>
            <small>{{ slot.config ? '已保存' : '空' }}</small>
          </button>
          <button type="button" title="保存当前视觉到此槽位" @click="saveVisualFxSlot(slot)">保存</button>
        </div>
      </div>
      <div class="music-fx-section">自定义颜色</div>
      <div class="music-fx-color-row">
        <label>
          <span>界面高亮</span>
          <input v-model="visualFx.uiAccentColor" type="color" />
        </label>
        <label>
          <span>视觉主色</span>
          <input v-model="visualFx.visualTintColor" type="color" />
        </label>
        <button class="music-fx-color-pick" type="button" @click="sampleCoverPalette">封面取色</button>
      </div>
      <div class="music-fx-color-row">
        <label>
          <span>Home 填充</span>
          <input v-model="visualFx.homeAccentColor" type="color" />
        </label>
        <label>
          <span>主页图标</span>
          <input v-model="visualFx.homeIconColor" type="color" />
        </label>
        <label>
          <span>视觉图标</span>
          <input v-model="visualFx.visualIconColor" type="color" />
        </label>
      </div>
      <div class="music-fx-color-row">
        <label>
          <span>背景颜色</span>
          <input v-model="visualFx.backgroundColor" type="color" />
        </label>
        <button class="music-fx-color-pick" type="button" @click="visualFx.backgroundColor = '#000000'">默认</button>
      </div>
      <div class="music-fx-section">主控</div>
      <div class="music-fx-actions">
        <button class="music-fx-action" :disabled="!curTrack" @click="showCustomLyric = true">自定义歌词</button>
        <button class="music-fx-action" :disabled="!coverUrl" @click="showCoverCrop = true">裁剪封面</button>
        <button class="music-fx-action" :disabled="!customCoverUrl" @click="clearCustomCover">清除封面</button>
        <button class="music-fx-action" @click="pickCustomBackground">背景媒体</button>
        <button class="music-fx-action" :disabled="!customBackgroundUrl" @click="clearCustomBackground">清除背景</button>
        <button class="music-fx-action" :disabled="!curTrack" @click="showTrackDetail = true">歌曲详情</button>
        <button class="music-fx-action" @click="toggleImmersiveMode">{{ immersiveMode ? '退出沉浸' : '沉浸模式' }}</button>
        <button :class="['music-fx-action', desktopLyricsOn ? 'on' : '']" @click="toggleDesktopLyrics">{{ desktopLyricsOn ? '关闭桌面歌词' : '桌面歌词' }}</button>
        <button class="music-fx-action" :disabled="!curTrack" @click="openCollectModal(curTrack)">收藏到歌单</button>
        <button :class="['music-fx-action', diySimpleMode ? 'on' : '']" @click="diySimpleMode = !diySimpleMode">{{ diySimpleMode ? '完整版' : '简约模式' }}</button>
        <button class="music-fx-action" @click="exportVisualFx">导出 FX</button>
        <button class="music-fx-action" @click="importVisualFx">导入 FX</button>
        <button class="music-fx-action" :disabled="beatAnalyzing || !curTrack" @click="showBeatModal = true">节拍分析</button>
        <button class="music-fx-action" @click="visualGuideIndex = 0; showVisualGuide = true">视觉引导</button>
        <button class="music-fx-action" @click="showHotkeyModal = true">热键设置</button>
      </div>
      <div v-if="desktopLyricsOn" class="music-fx-section">桌面歌词</div>
      <div v-if="desktopLyricsOn" class="music-fx-color-row">
        <label>
          <span>主歌词</span>
          <input v-model="desktopLyricPrimaryColor" type="color" />
        </label>
        <label>
          <span>下一行</span>
          <input v-model="desktopLyricSecondaryColor" type="color" />
        </label>
      </div>
      <label v-if="desktopLyricsOn" class="music-fx-slider">
        <span>桌面字号 <b>{{ desktopLyricFontSize }}</b></span>
        <input v-model.number="desktopLyricFontSize" max="72" min="24" step="1" type="range" />
      </label>
      <label v-if="desktopLyricsOn" class="music-fx-slider">
        <span>桌面歌词透明 <b>{{ desktopLyricOpacity.toFixed(2) }}</b></span>
        <input v-model.number="desktopLyricOpacity" max="1" min="0.28" step="0.01" type="range" />
      </label>
      <label v-if="desktopLyricsOn" class="music-fx-slider">
        <span>桌面歌词高度 <b>{{ desktopLyricY.toFixed(2) }}</b></span>
        <input v-model.number="desktopLyricY" max="0.92" min="0.08" step="0.01" type="range" />
      </label>
      <label v-if="desktopLyricsOn" class="music-fx-slider">
        <span>背景透明 <b>{{ desktopLyricBgOpacity.toFixed(2) }}</b></span>
        <input v-model.number="desktopLyricBgOpacity" max="0.9" min="0" step="0.01" type="range" />
      </label>
      <label v-if="desktopLyricsOn" class="music-fx-slider">
        <span>电影震动 <b>{{ desktopLyricShake.toFixed(2) }}</b></span>
        <input v-model.number="desktopLyricShake" max="1" min="0" step="0.01" type="range" />
      </label>
      <div v-if="desktopLyricsOn" class="music-fx-seg">
        <button :class="{ on: desktopLyricFps === 24 }" @click="desktopLyricFps = 24">24</button>
        <button :class="{ on: desktopLyricFps === 30 }" @click="desktopLyricFps = 30">30</button>
        <button :class="{ on: desktopLyricFps === 60 }" @click="desktopLyricFps = 60">60</button>
        <button :class="{ on: desktopLyricFps === 120 }" @click="desktopLyricFps = 120">120</button>
        <button :class="{ on: desktopLyricFps === 0 }" @click="desktopLyricFps = 0">无上限</button>
      </div>
      <div v-if="desktopLyricsOn" class="music-fx-seg">
        <button :class="{ on: desktopLyricFontFamily === 'system' }" @click="desktopLyricFontFamily = 'system'">系统</button>
        <button :class="{ on: desktopLyricFontFamily === 'serif' }" @click="desktopLyricFontFamily = 'serif'">衬线</button>
        <button :class="{ on: desktopLyricFontFamily === 'gothic' }" @click="desktopLyricFontFamily = 'gothic'">黑体</button>
      </div>
      <div v-if="desktopLyricsOn" class="music-fx-toggle-grid">
        <button :class="['music-fx-toggle', desktopLyricHighlightFollow ? 'on' : '']" @click="desktopLyricHighlightFollow = !desktopLyricHighlightFollow">高亮跟随</button>
        <button :class="['music-fx-toggle', desktopLyricsLocked ? 'on' : '']" @click="toggleDesktopLyricsLock">{{ desktopLyricsLocked ? '窗口已锁定' : '窗口可拖拽' }}</button>
        <button :class="['music-fx-toggle', desktopLyricsClickThrough ? 'on' : '']" @click="toggleDesktopLyricsClickThrough">{{ desktopLyricsClickThrough ? '穿透已开' : '点击穿透' }}</button>
      </div>
      <label class="music-fx-slider">
        <span>律动强度 <b>{{ visualFx.intensity.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.intensity" max="1.6" min="0.2" step="0.01" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>立体感 <b>{{ visualFx.depth.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.depth" max="1.8" min="0.2" step="0.01" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>封面清晰度 <b>{{ visualFx.coverResolution.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.coverResolution" max="1.55" min="0.72" step="0.01" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>镜头晃动 <b>{{ visualFx.cinemaShake.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.cinemaShake" max="1.8" min="0" step="0.01" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>歌词溢光 <b>{{ visualFx.lyricGlowStrength.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.lyricGlowStrength" max="0.85" min="0" step="0.01" type="range" />
      </label>
      <div v-show="!diySimpleMode" class="music-fx-advanced">
        <div class="music-fx-section">粒子</div>
        <label class="music-fx-slider">
          <span>粒子大小 <b>{{ visualFx.point.toFixed(2) }}</b></span>
          <input v-model.number="visualFx.point" max="1.8" min="0.45" step="0.01" type="range" />
        </label>
        <label class="music-fx-slider">
          <span>运动速度 <b>{{ visualFx.speed.toFixed(2) }}</b></span>
          <input v-model.number="visualFx.speed" max="2.5" min="0.2" step="0.01" type="range" />
        </label>
        <label class="music-fx-slider">
          <span>散射 <b>{{ visualFx.scatter.toFixed(2) }}</b></span>
          <input v-model.number="visualFx.scatter" max="1.2" min="0" step="0.01" type="range" />
        </label>
        <label class="music-fx-slider">
          <span>扭曲 <b>{{ visualFx.twist.toFixed(2) }}</b></span>
          <input v-model.number="visualFx.twist" max="1.8" min="0" step="0.01" type="range" />
        </label>
        <label class="music-fx-slider">
          <span>色彩张力 <b>{{ visualFx.colorTension.toFixed(2) }}</b></span>
          <input v-model.number="visualFx.colorTension" max="2" min="0.5" step="0.01" type="range" />
        </label>
        <label class="music-fx-slider">
          <span>辉光 <b>{{ visualFx.bloom.toFixed(2) }}</b></span>
          <input v-model.number="visualFx.bloom" max="1.3" min="0" step="0.01" type="range" />
        </label>
        <label class="music-fx-slider">
          <span>背景透明度 <b>{{ visualFx.backgroundOpacity.toFixed(2) }}</b></span>
          <input v-model.number="visualFx.backgroundOpacity" max="1" min="0.12" step="0.01" type="range" />
        </label>
        <label class="music-fx-slider">
          <span>背景压暗 <b>{{ visualFx.bgFade.toFixed(2) }}</b></span>
          <input v-model.number="visualFx.bgFade" max="1.2" min="0" step="0.01" type="range" />
        </label>
        <div class="music-fx-section">叠加效果</div>
        <div class="music-fx-toggle-grid">
          <button :class="['music-fx-toggle', visualFx.floatLayerEnabled ? 'on' : '']" @click="visualFx.floatLayerEnabled = !visualFx.floatLayerEnabled">浮空粒子</button>
          <button :class="['music-fx-toggle', visualFx.cinemaEnabled ? 'on' : '']" @click="visualFx.cinemaEnabled = !visualFx.cinemaEnabled">电影镜头</button>
          <button :class="['music-fx-toggle', visualFx.bloomEnabled ? 'on' : '']" @click="visualFx.bloomEnabled = !visualFx.bloomEnabled">粒子溢光</button>
          <button :class="['music-fx-toggle', visualFx.lyricGlowEnabled ? 'on' : '']" @click="visualFx.lyricGlowEnabled = !visualFx.lyricGlowEnabled">歌词溢光</button>
          <button :class="['music-fx-toggle', visualFx.beatGlowEnabled ? 'on' : '']" @click="visualFx.beatGlowEnabled = !visualFx.beatGlowEnabled">鼓点溢光</button>
          <button :class="['music-fx-toggle', visualFx.lyricGlowParticlesEnabled ? 'on' : '']" @click="visualFx.lyricGlowParticlesEnabled = !visualFx.lyricGlowParticlesEnabled">歌词光粒</button>
          <button :class="['music-fx-toggle', visualFx.lyricCameraLockEnabled ? 'on' : '']" @click="visualFx.lyricCameraLockEnabled = !visualFx.lyricCameraLockEnabled">歌词镜头绑定</button>
          <button :class="['music-fx-toggle', visualFx.edgeHighlightEnabled ? 'on' : '']" @click="visualFx.edgeHighlightEnabled = !visualFx.edgeHighlightEnabled">轮廓高亮</button>
          <button :class="['music-fx-toggle', visualFx.backCoverEnabled ? 'on' : '']" @click="visualFx.backCoverEnabled = !visualFx.backCoverEnabled">背面封面</button>
          <button :class="['music-fx-toggle', visualFx.colorLabEnabled ? 'on' : '']" @click="visualFx.colorLabEnabled = !visualFx.colorLabEnabled">颜色实验室</button>
          <button :class="['music-fx-toggle', visualFx.skullPresetEnabled ? 'on' : '']" @click="visualFx.skullPresetEnabled = !visualFx.skullPresetEnabled">Skull 预设</button>
          <button :class="['music-fx-toggle', visualFx.performanceManagerEnabled ? 'on' : '']" @click="visualFx.performanceManagerEnabled = !visualFx.performanceManagerEnabled">性能管理</button>
          <button :class="['music-fx-toggle', visualFx.liveBackgroundKeep ? 'on' : '']" @click="visualFx.liveBackgroundKeep = !visualFx.liveBackgroundKeep">直播后台保持</button>
          <button :class="['music-fx-toggle', visualFx.gestureControlEnabled ? 'on' : '']" @click="toggleGestureControl">手势 HUD</button>
          <button :class="['music-fx-toggle', visualFx.splashEnabled ? 'on' : '']" @click="visualFx.splashEnabled = !visualFx.splashEnabled">启动动画</button>
          <button :class="['music-fx-toggle', visualFx.wallpaperModeEnabled ? 'on' : '']" @click="visualFx.wallpaperModeEnabled = !visualFx.wallpaperModeEnabled">壁纸模式</button>
        </div>
      <label class="music-fx-slider">
        <span>玻璃色差 <b>{{ visualFx.glassChromaticOffset.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.glassChromaticOffset" max="2" min="0" step="0.01" type="range" />
      </label>
      <div class="music-fx-section">性能档位</div>
      <div class="music-fx-seg">
        <button :class="{ on: visualFx.renderQuality === 'auto' }" @click="visualFx.renderQuality = 'auto'">自动</button>
        <button :class="{ on: visualFx.renderQuality === 'keep' }" @click="visualFx.renderQuality = 'keep'">保持</button>
        <button :class="{ on: visualFx.renderQuality === 'release' }" @click="visualFx.renderQuality = 'release'">释放</button>
      </div>
      <div class="music-fx-subnote">{{ visualFx.liveBackgroundKeep ? '直播后台保持已开启，后台不会自动切到释放档。' : '后台时按性能档位自动降载。' }}</div>
      <div class="music-fx-section">歌词外观</div>
      <div class="music-fx-subsection">歌词来源</div>
      <div class="music-fx-seg">
        <button :class="{ on: visualFx.lyricSourceMode === 'original' }" @click="visualFx.lyricSourceMode = 'original'">原词</button>
        <button :class="{ on: visualFx.lyricSourceMode === 'custom' }" @click="visualFx.lyricSourceMode = 'custom'; showCustomLyric = true">自定义</button>
      </div>
      <div class="music-fx-subsection">歌词同步</div>
      <div class="music-fx-stepper">
        <button type="button" title="歌词慢 0.1 秒" @click="adjustLyricTimeOffset(-0.1)">-</button>
        <span>歌词校准 <b>{{ lyricTimeOffsetSec.toFixed(1) }}s</b></span>
        <button type="button" title="歌词快 0.1 秒" @click="adjustLyricTimeOffset(0.1)">+</button>
        <button type="button" title="重置歌词校准" @click="resetLyricTimeOffset">重置</button>
      </div>
      <div class="music-fx-color-row">
        <label>
          <span>普通</span>
          <input v-model="visualFx.lyricPrimaryColor" type="color" />
        </label>
        <label>
          <span>高亮</span>
          <input v-model="visualFx.lyricActiveColor" type="color" />
        </label>
        <label>
          <span>溢光</span>
          <input v-model="visualFx.lyricGlowColor" type="color" />
        </label>
      </div>
      <label class="music-fx-slider">
        <span>歌词大小 <b>{{ visualFx.lyricSize.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.lyricSize" max="1.65" min="0.35" step="0.01" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>行距 <b>{{ visualFx.lyricLineHeight.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.lyricLineHeight" max="1.35" min="0.86" step="0.01" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>字间距 <b>{{ visualFx.lyricLetterSpacing.toFixed(3) }}</b></span>
        <input v-model.number="visualFx.lyricLetterSpacing" max="0.18" min="-0.04" step="0.005" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>字重 <b>{{ visualFx.lyricWeight.toFixed(0) }}</b></span>
        <input v-model.number="visualFx.lyricWeight" max="900" min="500" step="50" type="range" />
      </label>
      <div class="music-fx-subsection">歌词字体</div>
      <div class="music-fx-seg">
        <button :class="{ on: visualFx.lyricFontFamily === 'system' }" @click="visualFx.lyricFontFamily = 'system'">系统</button>
        <button :class="{ on: visualFx.lyricFontFamily === 'serif' }" @click="visualFx.lyricFontFamily = 'serif'">衬线</button>
        <button :class="{ on: visualFx.lyricFontFamily === 'gothic' }" @click="visualFx.lyricFontFamily = 'gothic'">黑体</button>
        <button :class="{ on: visualFx.lyricFontFamily === 'mono' }" @click="visualFx.lyricFontFamily = 'mono'">等宽</button>
      </div>
      <label class="music-fx-slider">
        <span>歌词水平 <b>{{ visualFx.lyricOffsetX.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.lyricOffsetX" max="2" min="-2" step="0.01" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>歌词垂直 <b>{{ visualFx.lyricOffsetY.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.lyricOffsetY" max="1.35" min="-1.2" step="0.01" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>歌词前后 <b>{{ visualFx.lyricOffsetZ.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.lyricOffsetZ" max="1.6" min="-1.6" step="0.01" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>上下角度 <b>{{ visualFx.lyricRotateX.toFixed(0) }}°</b></span>
        <input v-model.number="visualFx.lyricRotateX" max="42" min="-42" step="1" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>左右角度 <b>{{ visualFx.lyricRotateY.toFixed(0) }}°</b></span>
        <input v-model.number="visualFx.lyricRotateY" max="42" min="-42" step="1" type="range" />
      </label>
      <div v-if="visualFx.colorLabEnabled" class="music-fx-color-lab">
        <button v-for="preset in COLOR_LAB_PRESETS" :key="preset.color" :style="{ '--lab-color': preset.color }" type="button" @click="applyCoverPalette(preset.color)">{{ preset.label }}</button>
        <button type="button" @click="sampleCoverPalette">封面取色</button>
      </div>
      <div class="music-fx-section">3D 歌单架</div>
      <div class="music-fx-seg">
        <button :class="{ on: visualFx.shelfMode === 'off' }" @click="visualFx.shelfMode = 'off'">关闭</button>
        <button :class="{ on: visualFx.shelfMode === 'side' }" @click="visualFx.shelfMode = 'side'">侧栏</button>
        <button :class="{ on: visualFx.shelfMode === 'stage' }" @click="visualFx.shelfMode = 'stage'">舞台</button>
      </div>
      <div class="music-fx-seg">
        <button :class="{ on: visualFx.shelfCameraMode === 'static' }" @click="setShelfCameraMode('static')">静态镜头</button>
        <button :class="{ on: visualFx.shelfCameraMode === 'dynamic' }" @click="setShelfCameraMode('dynamic')">动态镜头</button>
      </div>
      <div class="music-fx-seg">
        <button :class="{ on: visualFx.shelfPresence === 'auto' }" @click="visualFx.shelfPresence = 'auto'">自动隐藏</button>
        <button :class="{ on: visualFx.shelfPresence === 'always' }" @click="visualFx.shelfPresence = 'always'">常驻</button>
      </div>
      <div class="music-fx-toggle-grid">
        <button :class="['music-fx-toggle', visualFx.shelfShowFavorites ? 'on' : '']" @click="visualFx.shelfShowFavorites = !visualFx.shelfShowFavorites">显示收藏</button>
        <button :class="['music-fx-toggle', visualFx.shelfShowRecents ? 'on' : '']" @click="visualFx.shelfShowRecents = !visualFx.shelfShowRecents">显示最近</button>
        <button :class="['music-fx-toggle', visualFx.shelfShowPodcasts ? 'on' : '']" @click="visualFx.shelfShowPodcasts = !visualFx.shelfShowPodcasts">显示播客</button>
        <button :class="['music-fx-toggle', visualFx.shelfMergeCollections ? 'on' : '']" @click="visualFx.shelfMergeCollections = !visualFx.shelfMergeCollections">更多集合 / 合并滚动</button>
      </div>
      <div class="music-fx-subnote">滚轮切换歌单架卡片；开启更多集合后会加入专辑/艺人集合来源。</div>
      <div class="music-fx-color-row">
        <label>
          <span>歌单架</span>
          <input v-model="visualFx.shelfAccentColor" type="color" />
        </label>
        <button class="music-fx-color-pick" type="button" @click="sampleCoverPalette">封面取色</button>
      </div>
      <div v-if="coverPalette" class="music-fx-swatches">
        <button v-for="color in coverPalette.swatches" :key="color" :style="{ '--swatch': color }" type="button" :title="color" @click="applyCoverPalette(color)"></button>
      </div>
      <label class="music-fx-slider">
        <span>歌单架大小 <b>{{ visualFx.shelfSize.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.shelfSize" max="1.45" min="0.65" step="0.01" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>左右位置 <b>{{ visualFx.shelfOffsetX.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.shelfOffsetX" max="1.2" min="-1.2" step="0.01" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>上下位置 <b>{{ visualFx.shelfOffsetY.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.shelfOffsetY" max="0.9" min="-0.9" step="0.01" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>前后景深 <b>{{ visualFx.shelfOffsetZ.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.shelfOffsetZ" max="0.9" min="-0.9" step="0.01" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>侧向角度 <b>{{ visualFx.shelfAngleY.toFixed(0) }}°</b></span>
        <input v-model.number="visualFx.shelfAngleY" max="30" min="-30" step="1" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>歌单架透明度 <b>{{ visualFx.shelfOpacity.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.shelfOpacity" max="1" min="0.25" step="0.01" type="range" />
      </label>
      <label class="music-fx-slider">
        <span>背景透明度 <b>{{ visualFx.shelfBgOpacity.toFixed(2) }}</b></span>
        <input v-model.number="visualFx.shelfBgOpacity" max="0.98" min="0.25" step="0.01" type="range" />
      </label>
      </div>
    </div>

    <MusicQueuePanel
      v-model:filter="listFilter"
      v-model:panel-tab="panelTab"
      :current-key="currentKey"
      :fav-keys="favKeys"
      :favs="favs"
      :playlist="playlist"
      :recents="recents"
      :visible="showPanel"
      @clear-recents="recents = []; saveRecs()"
      @close="showPanel = false"
      @next-play="moveQueueNext"
      @play="playFrom"
      @remove-queue="removeQueueAt"
      @remove-fav="removeFav"
      @toggle-fav="toggleFav"
    />

    <MusicMiniQueue
      :current-index="curIdx"
      :tracks="playlist"
      :visible="showQueue"
      @close="showQueue = false"
      @play="(idx) => { showQueue = false; loadIdx(idx, true) }"
      @remove="removeQueueAt"
    />

    <MusicBottomConsole
      :album="album"
      :artist="artist"
      :cover-url="coverUrl"
      :display-time="dispTime"
      :duration-text="fmtTime(effectiveDur)"
      :err-msg="errMsg"
      :ext="ext"
      :index="curIdx"
      :is-fav="isFav"
      :loading="loading"
      :lyric-debug-text="lyricDebugText"
      :lyric-debug-title="lyricDebugTitle"
      :meta-load="metaLoad"
      :mode="mode"
      :mode-label="modeLbl"
      :mode-on="modeOn"
      :muted="muted"
      :playing="playing"
      :progress-percent="progPct"
      :show-lyrics="showLyrics"
      :show-fx-panel="showFxPanel"
      :show-panel="showPanel"
      :title="title"
      :total="playlist.length"
      :vol="vol"
      @cycle-mode="cycleMode"
      @next="playNext(false)"
      @prev="playPrev"
      @progress-down="onProgDown"
      @toggle-fav="toggleFav(curTrack)"
      @toggle-fx="showFxPanel = !showFxPanel"
      @toggle-lyrics="toggleLyrics"
      @toggle-mute="toggleMute"
      @toggle-panel="showPanel = !showPanel; showQueue = false"
      @toggle-play="togglePlay"
      @volume-down="onVolDown"
    />

    <CustomLyricModal
      :track-title="title"
      :visible="showCustomLyric"
      @close="showCustomLyric = false"
      @delete="deleteCustomLyric"
      @save="applyCustomLyric"
    />

    <CoverCropModal
      :image-url="coverUrl"
      :visible="showCoverCrop"
      @close="showCoverCrop = false"
      @commit="commitCoverCrop"
    />

    <div v-if="showVisualGuide" class="visual-guide-mask spotlight-guide-mask" :data-guide-step="visualGuideCurrent.spot" @click.self="showVisualGuide = false">
      <div :class="['visual-guide-spotlight', visualGuideCurrent.spot]"></div>
      <div class="visual-guide-card">
        <div class="visual-guide-head">
          <span>VISUAL GUIDE · {{ String(visualGuideIndex + 1).padStart(2, '0') }}/{{ String(visualGuideItems.length).padStart(2, '0') }}</span>
          <button type="button" @click="showVisualGuide = false">×</button>
        </div>
        <div class="visual-guide-list staged">
          <div class="visual-guide-item active">
            <b>{{ String(visualGuideIndex + 1).padStart(2, '0') }}</b>
            <div>
              <strong>{{ visualGuideCurrent.title }}</strong>
              <small>{{ visualGuideCurrent.text }}</small>
            </div>
          </div>
        </div>
        <div class="visual-guide-actions">
          <button type="button" :disabled="visualGuideIndex <= 0" @click="visualGuideIndex = Math.max(0, visualGuideIndex - 1)">上一项</button>
          <button type="button" @click="visualGuideIndex >= visualGuideItems.length - 1 ? showVisualGuide = false : visualGuideIndex += 1">
            {{ visualGuideIndex >= visualGuideItems.length - 1 ? '完成' : '下一项' }}
          </button>
        </div>
      </div>
    </div>

    <div v-if="showHotkeyModal" class="hotkey-modal-mask" @click.self="showHotkeyModal = false">
      <div class="hotkey-modal">
        <div class="hotkey-modal-head">
          <span>HOTKEYS</span>
          <button type="button" title="恢复默认" @click="resetHotkeys">↺</button>
          <button type="button" @click="showHotkeyModal = false">×</button>
        </div>
        <div class="hotkey-toolbar">
          <button :class="['music-fx-toggle', globalHotkeysEnabled ? 'on' : '']" type="button" @click="globalHotkeysEnabled = !globalHotkeysEnabled; configureGlobalHotkeys()">
            {{ globalHotkeysEnabled ? '全局热键已开' : '启用全局热键' }}
          </button>
          <span>{{ globalHotkeyStatus || '窗口热键始终生效，全局热键使用 Electron 注册。' }}</span>
        </div>
        <div class="hotkey-list">
          <div v-for="item in hotkeyItems" :key="item.id" :class="['hotkey-row', hasHotkeyConflict(hotkeyItems, item) ? 'conflict' : '']">
            <button class="hotkey-capture" type="button" @keydown.stop="captureHotkey($event, item)" @click="hotkeyCaptureId = item.id">
              {{ hotkeyCaptureId === item.id ? '按下快捷键' : item.value }}
            </button>
            <span>{{ item.label }}</span>
          </div>
        </div>
      </div>
    </div>

    <div v-if="showCollectModal" class="collect-modal-mask" @click.self="showCollectModal = false">
      <div class="collect-modal">
        <div class="collect-modal-head">
          <div>
            <span>COLLECT TO PLAYLIST</span>
            <small>{{ collectTargetTitle }}</small>
          </div>
          <button type="button" @click="showCollectModal = false">×</button>
        </div>
        <div class="collect-new-row">
          <input v-model="collectNewPlaylistName" placeholder="新建歌单名称" @keydown.enter="collectCurrentToNewPlaylist" />
          <button type="button" @click="collectCurrentToNewPlaylist">新建并收藏</button>
        </div>
        <div class="collect-list">
          <button v-for="list in localPlaylists" :key="list.id" type="button" @click="collectCurrentToPlaylist(list); showCollectModal = false">
            <span>{{ list.name }}</span>
            <small>{{ list.tracks.length }} 首</small>
          </button>
          <div v-if="!localPlaylists.length" class="collect-empty">暂无本地歌单，可先新建一个。</div>
        </div>
      </div>
    </div>

    <div v-if="showBeatModal" class="beat-modal-mask" @click.self="showBeatModal = false">
      <div class="beat-modal">
        <div class="beat-modal-head">
          <span>LOCAL BEAT ANALYSIS</span>
          <button type="button" @click="showBeatModal = false">×</button>
        </div>
        <div class="beat-modal-copy">
          只分析当前正在播放的网盘或本地音频，用于驱动粒子脉冲、电影镜头和鼓点辉光，不会接入网易云或 QQ 音源。首次分析需要完整读取和解码音频，FLAC 可能需要 1-2 分钟，完成后会使用本地缓存。
        </div>
        <div class="beat-modal-grid">
          <button type="button" :disabled="beatAnalyzing || !curTrack" @click="showBeatModal = false; analyzeCurrentBeat('mr')">
            <b>BP Beat</b>
            <span>适合常规歌曲，快速检测 BPM 和峰值节拍。</span>
          </button>
          <button type="button" :disabled="beatAnalyzing || !curTrack" @click="showBeatModal = false; analyzeCurrentBeat('dj')">
            <b>DJ Beat</b>
            <span>适合长混音和强节奏曲目，峰值更密集。</span>
          </button>
        </div>
        <div v-if="beatInfo" class="beat-modal-status">
          最近结果：{{ beatInfo.mode === 'dj' ? 'DJ Beat' : 'BP Beat' }} · {{ beatInfo.bpm }} BPM · {{ beatInfo.peaks }} peaks
        </div>
      </div>
    </div>

    <div v-if="showTrackDetail" class="music-detail-mask" @click.self="showTrackDetail = false">
      <div class="music-detail-modal">
        <div class="music-detail-head">
          <div>
            <div class="music-detail-title">{{ title || '未命名歌曲' }}</div>
            <div class="music-detail-sub">{{ artist || '未知艺人' }}<span v-if="album"> · {{ album }}</span></div>
          </div>
          <button class="music-detail-close" @click="showTrackDetail = false">×</button>
        </div>
        <div class="music-detail-hero">
          <img v-if="coverUrl" :src="coverUrl" alt="" />
          <div v-else class="music-detail-cover-empty">♪</div>
          <div class="music-detail-grid">
            <span>文件</span><b>{{ curTrack?.file_name || '' }}</b>
            <span>格式</span><b>{{ ext || '未知' }}</b>
            <span>时长</span><b>{{ fmtTime(dur) }}</b>
            <span>来源</span><b>{{ curTrack ? `${curTrack.user_id} · ${curTrack.drive_id}` : '' }}</b>
            <span>歌词</span><b>{{ lyricDebugText || (hasLyrics ? `已解析 ${lyricLines.length} 行` : '暂无歌词') }}</b>
            <span>元数据</span><b>{{ (meta?.metadataSources || ['filename']).join(', ') }}</b>
          </div>
        </div>
      </div>
    </div>

    <audio
      ref="audioRef"
      preload="auto"
      style="display:none"
      @durationchange="onLoaded"
      @ended="onEnded"
      @error="onErr"
      @loadedmetadata="onLoaded"
      @pause="onPause"
      @play="onPlay"
      @timeupdate="onTime"
    />
  </div>
</template>

<style scoped lang="less">
.mineradio-player {
  position: relative;
  width: 100%;
  height: 100vh;
  overflow: hidden;
  --music-stage-bg: #050608;
  --music-stage-text: #fff;
  --music-stage-muted: rgba(255,255,255,.56);
  --music-stage-subtle: rgba(255,255,255,.28);
  --music-lyric-idle: rgba(255,255,255,.26);
  --music-lyric-hover: rgba(255,255,255,.72);
  --music-lyric-active: #fff9e8;
  --music-lyric-past: rgba(255,255,255,.14);
  --music-stage-base: #08090b;
  --music-stage-bg-filter: blur(120px) brightness(.18) saturate(1.5);
  --music-stage-bg-opacity: .92;
  --music-particle-blend: screen;
  --music-particle-filter: drop-shadow(0 0 22px rgba(143,245,234,.34));
  --music-stage-aura:
    radial-gradient(circle at 50% 36%, rgba(0,245,212,.18), transparent 30%),
    radial-gradient(circle at 72% 22%, rgba(244,210,138,.13), transparent 24%),
    radial-gradient(circle at 18% 70%, rgba(36,66,255,.15), transparent 32%);
  --music-stage-vignette:
    radial-gradient(circle at 50% 34%, rgba(255,255,255,.06), transparent 30%),
    linear-gradient(180deg, rgba(8,9,11,.06), rgba(8,9,11,.24) 56%, rgba(8,9,11,.74)),
    linear-gradient(90deg, rgba(8,9,11,.48), transparent 22%, transparent 78%, rgba(8,9,11,.48));
  --music-console-bg: rgba(4,7,10,.34);
  --music-console-border: rgba(255,255,255,.24);
  --music-console-shadow:
    inset 0 1px 0 rgba(255,255,255,.24),
    inset 0 0 0 1px rgba(255,255,255,.10),
    0 20px 60px rgba(0,0,0,.24);
  --music-console-hover: rgba(255,255,255,.12);
  --music-console-tag-bg: rgba(255,255,255,.075);
  --music-console-track: rgba(255,255,255,.16);
  --music-console-sleep-text: rgba(255,255,255,.78);
  color: var(--music-stage-text);
  background: var(--music-stage-bg);
  user-select: none;
  -webkit-font-smoothing: antialiased;
}
.control-glass-filter-svg {
  position: absolute;
  width: 0;
  height: 0;
  overflow: hidden;
  pointer-events: none;
}
body:not([arco-theme='dark']) .mineradio-player {
  --music-stage-bg: #d9e4e8;
  --music-stage-text: #14202a;
  --music-stage-muted: rgba(20,32,42,.62);
  --music-stage-subtle: rgba(20,32,42,.38);
  --music-lyric-idle: rgba(20,32,42,.38);
  --music-lyric-hover: rgba(20,32,42,.78);
  --music-lyric-active: #13222b;
  --music-lyric-past: rgba(20,32,42,.22);
  --music-stage-base: #dce8ec;
  --music-stage-bg-filter: blur(92px) brightness(1.08) saturate(1.24);
  --music-stage-bg-opacity: .68;
  --music-particle-blend: normal;
  --music-particle-filter: drop-shadow(0 0 16px rgba(0,128,120,.18));
  --music-stage-aura:
    radial-gradient(circle at 46% 30%, rgba(0,180,168,.22), transparent 32%),
    radial-gradient(circle at 74% 22%, rgba(226,167,76,.18), transparent 24%),
    radial-gradient(circle at 18% 72%, rgba(78,105,200,.16), transparent 34%);
  --music-stage-vignette:
    radial-gradient(circle at 50% 34%, rgba(255,255,255,.18), transparent 32%),
    linear-gradient(180deg, rgba(240,248,250,.08), rgba(214,226,231,.24) 58%, rgba(205,219,226,.54)),
    linear-gradient(90deg, rgba(205,218,224,.42), transparent 22%, transparent 78%, rgba(205,218,224,.42));
  --music-console-bg: rgba(255,255,255,.34);
  --music-console-border: rgba(255,255,255,.62);
  --music-console-shadow:
    inset 0 1px 0 rgba(255,255,255,.74),
    inset 0 0 0 1px rgba(255,255,255,.34),
    0 18px 54px rgba(55,78,94,.18);
  --music-console-hover: rgba(255,255,255,.54);
  --music-console-tag-bg: rgba(255,255,255,.46);
  --music-console-track: rgba(20,32,42,.14);
  --music-console-sleep-text: rgba(20,32,42,.78);
}
.mineradio-player.embedded {
  height: 100%;
}
.mineradio-player.side-panel :deep(.bottom-bar) {
  width: calc(100% - 22px);
  bottom: 10px;
  padding: 8px 10px 10px;
  border-radius: 18px;
}
.mineradio-player.side-panel :deep(.controls) {
  grid-template-columns: 1fr;
  gap: 7px;
}
.mineradio-player.side-panel :deep(.control-track),
.mineradio-player.side-panel :deep(.control-cluster.modes) {
  justify-content: center;
}
.mineradio-player.side-panel :deep(.volume-control),
.mineradio-player.side-panel :deep(.source-control),
.mineradio-player.side-panel :deep(.control-status) {
  display: none;
}
.mineradio-player.side-panel :deep(.control-title) {
  max-width: 220px;
}
.mineradio-player.side-panel :deep(.music-cover-shell) {
  width: min(188px, 54vw, 34vh);
}
.mineradio-player.side-panel :deep(.music-cover-wrap) {
  padding-bottom: 176px;
}
.mineradio-player.side-panel :deep(.music-lyric-stage) {
  padding: 24px 18px 176px;
}
.mineradio-player.side-panel :deep(.music-lyric-line) {
  font-size: 18px;
}
.mineradio-player.immersive.controls-hidden :deep(.bottom-bar),
.mineradio-player.immersive.controls-hidden :deep(.bottom-handle) {
  opacity: 0;
  transform: translateX(-50%) translateY(88px) scale(.972);
  pointer-events: none;
}
.mineradio-player.immersive.controls-hidden .music-fx-panel,
.mineradio-player.immersive.controls-hidden .mineradio-titlebar {
  opacity: 0;
  pointer-events: none;
}
.mineradio-player.immersive :deep(.music-stage-copy) {
  opacity: .72;
}
.mineradio-player.simple-mode .music-fx-panel {
  width: min(380px, calc(100vw - 48px));
}
.mineradio-player.simple-mode .music-fx-presets,
.mineradio-player.simple-mode .music-fx-user-slots {
  display: none;
}
.mineradio-player.simple-mode :deep(.bottom-bar) {
  width: min(780px, calc(100vw - 42px));
}
.desktop-lyric-preview {
  position: absolute;
  z-index: 6;
  left: 50%;
  bottom: 132px;
  width: min(720px, calc(100vw - 160px));
  min-height: 78px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  grid-template-areas:
    "active tools"
    "next tools";
  gap: 3px 18px;
  align-items: center;
  padding: 13px 16px 13px 20px;
  border: 1px solid rgba(255,255,255,.08);
  border-radius: 22px;
  color: rgba(255,255,255,.92);
  background:
    radial-gradient(circle at 0 0, rgba(0,245,212,.12), transparent 36%),
    linear-gradient(145deg, rgba(9,11,15,.46), rgba(7,8,12,.66));
  box-shadow: 0 22px 70px rgba(0,0,0,.32), inset 0 1px 0 rgba(255,255,255,.08);
  backdrop-filter: blur(26px) saturate(1.18);
  opacity: 0;
  pointer-events: none;
  transform: translateX(-50%) translateY(10px);
  transition: opacity .8s ease, transform .8s cubic-bezier(.16,1,.3,1);
}
.desktop-lyric-preview.visible {
  opacity: 1;
  pointer-events: auto;
  transform: translateX(-50%) translateY(0);
}
.desktop-lyric-line {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.desktop-lyric-line.active {
  grid-area: active;
  color: #fff9e8;
  font-size: clamp(20px, 2.5vw, 34px);
  font-weight: 840;
  letter-spacing: .02em;
  text-shadow: 0 0 18px rgba(244,210,138,.20), 0 0 42px rgba(0,245,212,.14), 0 16px 44px rgba(0,0,0,.55);
}
.desktop-lyric-line.next {
  grid-area: next;
  color: rgba(255,255,255,.38);
  font-size: 13px;
  font-weight: 680;
}
.desktop-lyric-tools {
  grid-area: tools;
  display: grid;
  gap: 7px;
}
.desktop-lyric-tools button {
  height: 26px;
  padding: 0 10px;
  border: 1px solid rgba(255,255,255,.08);
  border-radius: 999px;
  color: rgba(255,255,255,.62);
  background: rgba(255,255,255,.045);
  font-size: 10px;
  font-weight: 760;
  cursor: pointer;
}
.desktop-lyric-tools button:hover {
  border-color: rgba(0,245,212,.26);
  color: #fff;
  background: rgba(0,245,212,.08);
}
.mineradio-player.controls-visible .desktop-lyric-preview,
.mineradio-player.side-panel .desktop-lyric-preview {
  opacity: 0;
  pointer-events: none;
  transform: translateX(-50%) translateY(12px);
}
.mineradio-splash {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: grid;
  place-items: center;
  background:
    linear-gradient(115deg, transparent 0%, rgba(255,83,103,.055) 24%, transparent 42%, rgba(244,210,138,.052) 62%, transparent 82%),
    repeating-linear-gradient(90deg, rgba(255,255,255,.030) 0 1px, transparent 1px 54px),
    repeating-linear-gradient(0deg, rgba(255,255,255,.020) 0 1px, transparent 1px 46px),
    radial-gradient(circle at 50% 48%, rgba(0,245,212,.16), transparent 30%),
    linear-gradient(180deg, #020606 0%, #050607 42%, #000 100%);
  pointer-events: none;
  animation: mineradioSplashFade 2.1s ease forwards;
  overflow: hidden;
}
.mineradio-splash::after {
  content: '';
  position: absolute;
  inset: 0;
  background:
    linear-gradient(90deg, rgba(0,0,0,.82), transparent 21%, transparent 79%, rgba(0,0,0,.82)),
    linear-gradient(180deg, rgba(0,0,0,.68), transparent 32%, transparent 64%, rgba(0,0,0,.74));
}
.mineradio-splash-noise {
  position: absolute;
  inset: 0;
  opacity: .04;
  background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 180 180' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='.9' numOctaves='2' stitchTiles='stitch'/%3E%3CfeColorMatrix type='saturate' values='0'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='.55'/%3E%3C/svg%3E");
  background-size: 180px 180px;
  mix-blend-mode: screen;
}
.mineradio-splash-wordmark {
  position: relative;
  z-index: 2;
  width: min(940px, 82vw);
  height: clamp(70px, 12vw, 136px);
  font-size: clamp(38px, 7vw, 104px);
  font-weight: 760;
  letter-spacing: -.045em;
  text-shadow: 0 0 28px rgba(0,245,212,.22), 0 22px 78px rgba(0,0,0,.56);
  animation: mineradioSplashWord 1.7s cubic-bezier(.16,1,.3,1) forwards;
}
.splash-word-mine,
.splash-word-radio {
  position: absolute;
  top: 50%;
  left: 50%;
  white-space: nowrap;
  transform: translate(-50%, -50%);
}
.splash-word-mine {
  color: #f8f8f2;
  animation: mineradioSplashMine 1.9s cubic-bezier(.16,1,.3,1) forwards;
}
.splash-word-radio {
  background: linear-gradient(94deg, rgba(255,255,255,.06), #fff 26%, rgba(244,210,138,.98) 48%, rgba(122,215,194,.90) 68%, rgba(255,255,255,.82));
  background-size: 300% 100%;
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
  -webkit-text-fill-color: transparent;
  animation: mineradioSplashRadio 1.9s cubic-bezier(.16,1,.3,1) forwards;
}
.mineradio-splash-line {
  position: absolute;
  z-index: 2;
  left: 50%;
  top: calc(50% + 78px);
  width: min(520px, 58vw);
  height: 2px;
  background: linear-gradient(90deg, transparent, rgba(0,245,212,.88), rgba(244,210,138,.78), transparent);
  box-shadow: 0 0 18px rgba(244,210,138,.24), 0 0 34px rgba(122,215,194,.10);
  transform: translateX(-50%) scaleX(.2);
  transform-origin: center;
  animation: mineradioSplashLine 1.8s cubic-bezier(.16,1,.3,1) forwards;
}
.mineradio-splash-enter {
  position: absolute;
  z-index: 2;
  left: 50%;
  top: calc(50% + 106px);
  color: rgba(255,255,255,.62);
  font-size: 11px;
  font-weight: 780;
  letter-spacing: .24em;
  transform: translateX(-50%);
  text-shadow: 0 0 18px rgba(244,210,138,.24), 0 0 34px rgba(122,215,194,.12);
  animation: mineradioSplashEnter 1.8s ease-in-out infinite alternate;
}
@keyframes mineradioSplashFade {
  0%, 72% { opacity: 1; }
  100% { opacity: 0; }
}
@keyframes mineradioSplashWord {
  0% { opacity: 0; transform: translateY(18px) scale(.96); filter: blur(16px); }
  32% { opacity: 1; filter: blur(0); }
  100% { opacity: .86; transform: translateY(0) scale(1); }
}
@keyframes mineradioSplashMine {
  0% { opacity: 0; clip-path: inset(48% 0 49% 0); transform: translate(calc(-50% - 10px), -42%) skewX(-10deg) scaleX(1.08); letter-spacing: .055em; }
  46% { opacity: 1; clip-path: inset(0); transform: translate(-50%, -50%) skewX(0) scaleX(1); letter-spacing: -.038em; }
  100% { opacity: 1; transform: translate(calc(-50% - clamp(62px, 9vw, 128px)), -50%) scale(.998); }
}
@keyframes mineradioSplashRadio {
  0%, 28% { opacity: 0; clip-path: inset(52% 0 44% 0); transform: translate(calc(-50% + clamp(70px, 11vw, 130px)), -50%) skewX(9deg) scaleX(1.06); background-position: 0 0; }
  58% { opacity: .9; clip-path: inset(20% 0 18% 0); background-position: 52% 0; }
  100% { opacity: 1; clip-path: inset(0); transform: translate(calc(-50% + clamp(190px, 23vw, 286px)), -50%) scale(1); background-position: 100% 0; }
}
@keyframes mineradioSplashLine {
  0% { opacity: 0; transform: translateX(-50%) scaleX(.08); }
  38% { opacity: 1; }
  100% { opacity: .38; transform: translateX(-50%) scaleX(1); }
}
@keyframes mineradioSplashEnter {
  from { opacity: .34; transform: translateX(-50%) translateY(4px); }
  to { opacity: .78; transform: translateX(-50%) translateY(0); }
}
.music-beat-chip {
  position: fixed;
  top: 58px;
  right: 24px;
  z-index: 24;
  display: grid;
  gap: 2px;
  min-width: 92px;
  padding: 9px 12px;
  border: 1px solid rgba(0,245,212,.22);
  border-radius: 999px;
  color: var(--music-stage-text);
  background: linear-gradient(145deg, rgba(10,18,22,.58), rgba(3,5,8,.72));
  box-shadow: 0 16px 44px rgba(0,0,0,.25), inset 0 1px 0 rgba(255,255,255,.10);
  backdrop-filter: blur(22px) saturate(1.2);
  pointer-events: none;
}
.music-beat-chip span {
  font-size: 12px;
  font-weight: 860;
}
.music-beat-chip small {
  color: var(--music-stage-muted);
  font-size: 9px;
  font-weight: 820;
  letter-spacing: .14em;
  text-transform: uppercase;
}
.music-fx-panel {
  position: absolute;
  z-index: 18;
  right: -460px;
  top: 64px;
  bottom: 132px;
  width: min(444px, calc(100vw - 48px));
  max-height: none;
  overflow: auto;
  overscroll-behavior: contain;
  padding: 18px 18px 20px;
  border: 1px solid rgba(0,245,212,.16);
  border-radius: 20px;
  color: var(--music-stage-text);
  background: rgba(8,11,15,.48);
  box-shadow: 0 24px 80px rgba(0,0,0,.38), inset 0 1px 0 rgba(255,255,255,.12);
  backdrop-filter: blur(44px) saturate(1.34);
  opacity: 0;
  pointer-events: none;
  transform: translateY(16px) scale(.97);
  transition: right .55s cubic-bezier(.16,1,.3,1), opacity .45s cubic-bezier(.16,1,.3,1), transform .55s cubic-bezier(.16,1,.3,1);
  scrollbar-width: thin;
  scrollbar-color: rgba(0,245,212,.28) transparent;
}
.music-fx-panel.show {
  right: 24px;
  opacity: 1;
  pointer-events: auto;
  transform: translateY(0) scale(1);
}
.music-fx-panel::-webkit-scrollbar {
  width: 3px;
}
.music-fx-panel::-webkit-scrollbar-thumb {
  border-radius: 3px;
  background: rgba(0,245,212,.28);
}
.music-fx-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.music-fx-title {
  font-size: 18px;
  font-weight: 840;
}
.music-fx-sub {
  margin-top: 4px;
  color: var(--music-stage-muted);
  font-size: 10px;
  font-weight: 760;
  letter-spacing: .12em;
}
.music-fx-presets {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  margin-top: 16px;
}
.music-fx-preset {
  min-height: 40px;
  border: 1px solid rgba(255,255,255,.09);
  border-radius: 12px;
  color: var(--music-stage-text);
  background:
    radial-gradient(circle at 18% 12%, rgba(0,245,212,.10), transparent 42%),
    rgba(255,255,255,.04);
  font-size: 12px;
  font-weight: 760;
  cursor: pointer;
  transition: transform .18s, border-color .18s, background .18s, box-shadow .18s;
}
.music-fx-preset:hover {
  transform: translateY(-1px);
  border-color: rgba(244,210,138,.28);
  background:
    radial-gradient(circle at 18% 12%, rgba(244,210,138,.14), transparent 42%),
    rgba(255,255,255,.065);
  box-shadow: 0 14px 32px rgba(0,0,0,.18);
}
.music-fx-user-slots {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  margin-top: 8px;
}
.music-fx-slot {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 48px;
  gap: 6px;
  min-width: 0;
}
.music-fx-slot button {
  min-width: 0;
  min-height: 38px;
  border: 1px solid rgba(255,255,255,.085);
  border-radius: 12px;
  color: var(--music-stage-muted);
  background: rgba(255,255,255,.035);
  font-size: 11px;
  font-weight: 760;
  cursor: pointer;
  transition: color .18s, background .18s, border-color .18s, transform .18s;
}
.music-fx-slot button:first-child {
  display: flex;
  min-width: 0;
  flex-direction: column;
  align-items: flex-start;
  justify-content: center;
  padding: 6px 10px;
}
.music-fx-slot button span,
.music-fx-slot button small {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.music-fx-slot button small {
  color: var(--music-stage-subtle);
  font-size: 10px;
}
.music-fx-slot button:hover {
  color: var(--music-stage-text);
  border-color: rgba(0,245,212,.28);
  background: rgba(0,245,212,.075);
  transform: translateY(-1px);
}
.music-fx-reset {
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  border: 1px solid rgba(255,255,255,.13);
  border-radius: 12px;
  color: var(--music-stage-muted);
  background: rgba(255,255,255,.055);
  cursor: pointer;
}
.music-fx-reset:hover {
  color: var(--music-stage-text);
  border-color: rgba(0,245,212,.32);
  background: rgba(0,245,212,.08);
}
.music-fx-section {
  margin: 18px 0 10px;
  color: var(--music-stage-muted);
  font-size: 11px;
  font-weight: 820;
  letter-spacing: .08em;
}
.music-fx-subsection {
  margin: 10px 0 6px;
  color: rgba(244,210,138,.68);
  font-size: 10px;
  font-weight: 780;
  letter-spacing: .08em;
}
.music-fx-actions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  margin-bottom: 8px;
}
.music-fx-action {
  height: 34px;
  border: 1px solid rgba(255,255,255,.11);
  border-radius: 11px;
  color: var(--music-stage-muted);
  background: rgba(255,255,255,.045);
  font-size: 12px;
  font-weight: 720;
  cursor: pointer;
  transition: background .18s, border-color .18s, color .18s, transform .18s;
}
.music-fx-action:hover {
  color: var(--music-stage-text);
  border-color: rgba(0,245,212,.30);
  background: rgba(0,245,212,.08);
  transform: translateY(-1px);
}
.music-fx-action.on {
  color: #eafffb;
  border-color: rgba(0,245,212,.34);
  background: rgba(0,245,212,.10);
  box-shadow: inset 0 1px 0 rgba(255,255,255,.08), 0 10px 28px rgba(0,245,212,.06);
}
.music-fx-action:disabled {
  opacity: .36;
  cursor: default;
  transform: none;
}
.music-fx-toggle-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  margin: 8px 0;
}
.music-fx-toggle {
  height: 36px;
  border: 1px solid rgba(255,255,255,.09);
  border-radius: 12px;
  color: var(--music-stage-muted);
  background: rgba(255,255,255,.038);
  font-size: 12px;
  font-weight: 760;
  cursor: pointer;
  transition: color .18s, background .18s, border-color .18s, box-shadow .18s, transform .18s;
}
.music-fx-toggle:hover {
  color: var(--music-stage-text);
  transform: translateY(-1px);
}
.music-fx-toggle.on {
  color: var(--music-stage-text);
  border-color: rgba(0,245,212,.32);
  background: rgba(0,245,212,.11);
  box-shadow: inset 0 1px 0 rgba(255,255,255,.12), 0 10px 28px rgba(0,245,212,.07);
}
.music-fx-color-row {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
  margin: 8px 0 4px;
}
.music-fx-color-row label {
  display: flex;
  min-width: 0;
  height: 42px;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 0 8px 0 11px;
  border: 1px solid rgba(255,255,255,.09);
  border-radius: 12px;
  color: var(--music-stage-muted);
  background: rgba(255,255,255,.038);
  font-size: 11px;
  font-weight: 760;
}
.music-fx-color-row input[type='color'] {
  width: 24px;
  height: 24px;
  flex: 0 0 auto;
  padding: 0;
  border: 1px solid rgba(255,255,255,.22);
  border-radius: 50%;
  overflow: hidden;
  background: transparent;
  cursor: pointer;
}
.music-fx-color-row input[type='color']::-webkit-color-swatch-wrapper {
  padding: 0;
}
.music-fx-color-row input[type='color']::-webkit-color-swatch {
  border: 0;
  border-radius: 50%;
}
.music-fx-color-pick {
  height: 42px;
  border: 1px solid rgba(244,210,138,.18);
  border-radius: 12px;
  color: rgba(244,210,138,.88);
  background: rgba(244,210,138,.07);
  font-size: 11px;
  font-weight: 820;
  cursor: pointer;
}
.music-fx-color-pick:hover {
  color: #fff0bf;
  border-color: rgba(244,210,138,.36);
  background: rgba(244,210,138,.12);
}
.music-fx-swatches {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 7px;
  margin: 7px 0 2px;
}
.music-fx-swatches button {
  height: 24px;
  border: 1px solid rgba(255,255,255,.16);
  border-radius: 8px;
  background: var(--swatch, #9db8cf);
  box-shadow: inset 0 1px 0 rgba(255,255,255,.22), 0 8px 18px rgba(0,0,0,.18);
  cursor: pointer;
}
.music-fx-swatches button:hover {
  transform: translateY(-1px);
}
.music-fx-color-lab {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 7px;
  margin: 8px 0 4px;
}
.music-fx-color-lab button {
  min-height: 32px;
  border: 1px solid color-mix(in srgb, var(--lab-color, #00f5d4) 28%, rgba(255,255,255,.09));
  border-radius: 10px;
  color: rgba(255,255,255,.72);
  background: linear-gradient(135deg, color-mix(in srgb, var(--lab-color, #00f5d4) 18%, transparent), rgba(255,255,255,.035));
  font-size: 11px;
  font-weight: 780;
  box-shadow: inset 0 1px 0 rgba(255,255,255,.14);
  cursor: pointer;
}
.music-fx-color-lab button:hover {
  border-color: color-mix(in srgb, var(--lab-color, #00f5d4) 56%, rgba(255,255,255,.18));
  color: #eafffb;
  background: linear-gradient(135deg, color-mix(in srgb, var(--lab-color, #00f5d4) 30%, transparent), rgba(255,255,255,.055));
}
.music-fx-subnote {
  margin: -2px 0 10px;
  padding: 8px 10px;
  border: 1px solid rgba(255,255,255,.07);
  border-radius: 12px;
  color: rgba(255,255,255,.42);
  background: rgba(255,255,255,.025);
  font-size: 10.5px;
  line-height: 1.45;
}
.visual-guide-mask,
.hotkey-modal-mask,
.beat-modal-mask,
.collect-modal-mask {
  position: fixed;
  inset: 0;
  z-index: 42;
  display: grid;
  place-items: center;
  padding: 28px;
  background: rgba(0,0,0,.36);
  backdrop-filter: blur(8px);
}
.spotlight-guide-mask {
  background:
    radial-gradient(circle at var(--spot-x, 50%) var(--spot-y, 50%), transparent 0 82px, rgba(0,0,0,.18) 118px, rgba(0,0,0,.58) 100%),
    rgba(0,0,0,.18);
  backdrop-filter: blur(4px);
}
.visual-guide-spotlight {
  position: fixed;
  z-index: -1;
  width: 168px;
  height: 168px;
  border: 1px solid rgba(0,245,212,.28);
  border-radius: 50%;
  box-shadow: 0 0 0 9999px rgba(0,0,0,.18), 0 0 42px rgba(0,245,212,.12), inset 0 0 30px rgba(0,245,212,.08);
  pointer-events: none;
}
.visual-guide-spotlight.one {
  left: 22px;
  bottom: 86px;
}
.visual-guide-spotlight.two {
  right: 10px;
  bottom: 92px;
}
.visual-guide-spotlight.three {
  right: 286px;
  top: 72px;
}
.visual-guide-spotlight.stage {
  left: 50%;
  top: 38%;
  width: 260px;
  height: 260px;
  transform: translate(-50%, -50%);
}
.visual-guide-spotlight.bottom {
  left: 50%;
  bottom: 42px;
  width: min(900px, calc(100vw - 80px));
  height: 128px;
  border-radius: 42px;
  transform: translateX(-50%);
}
.visual-guide-spotlight.shelf {
  right: 40px;
  top: 170px;
  width: 270px;
  height: 430px;
  border-radius: 38px;
}
.visual-guide-spotlight.fx {
  right: 12px;
  bottom: 96px;
  width: 420px;
  height: 520px;
  border-radius: 32px;
}
.visual-guide-card,
.hotkey-modal,
.beat-modal,
.collect-modal {
  width: min(460px, calc(100vw - 42px));
  border: 1px solid rgba(0,245,212,.18);
  border-radius: 22px;
  background:
    radial-gradient(circle at 0% 0%, rgba(0,245,212,.12), transparent 44%),
    linear-gradient(145deg, rgba(18,22,28,.88), rgba(5,6,9,.94));
  box-shadow: 0 34px 110px rgba(0,0,0,.55), inset 0 1px 0 rgba(255,255,255,.10);
  color: var(--music-stage-text);
  overflow: hidden;
}
.visual-guide-head,
.hotkey-modal-head,
.beat-modal-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 18px;
  border-bottom: 1px solid rgba(255,255,255,.08);
  color: rgba(255,255,255,.72);
  font-size: 11px;
  font-weight: 860;
  letter-spacing: .18em;
}
.visual-guide-head button,
.hotkey-modal-head button,
.beat-modal-head button {
  width: 28px;
  height: 28px;
  border: 1px solid rgba(255,255,255,.10);
  border-radius: 50%;
  color: rgba(255,255,255,.62);
  background: rgba(255,255,255,.045);
  cursor: pointer;
}
.hotkey-list {
  display: grid;
  gap: 8px;
  padding: 16px;
}
.hotkey-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px 0;
}
.hotkey-toolbar .music-fx-toggle {
  min-width: 128px;
}
.hotkey-toolbar span {
  color: rgba(255,255,255,.45);
  font-size: 11px;
  line-height: 1.5;
}
.hotkey-row {
  display: grid;
  grid-template-columns: 128px minmax(0, 1fr);
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid rgba(255,255,255,.08);
  border-radius: 13px;
  background: rgba(255,255,255,.04);
}
.hotkey-row.conflict {
  border-color: rgba(255,94,130,.42);
  background: rgba(255,94,130,.075);
}
.hotkey-row kbd {
  display: inline-flex;
  min-height: 28px;
  align-items: center;
  justify-content: center;
  padding: 0 9px;
  border: 1px solid rgba(0,245,212,.20);
  border-radius: 9px;
  color: rgba(0,245,212,.88);
  background: rgba(0,245,212,.065);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 11px;
  font-weight: 860;
}
.hotkey-capture {
  display: inline-flex;
  min-height: 28px;
  align-items: center;
  justify-content: center;
  padding: 0 9px;
  border: 1px solid rgba(0,245,212,.20);
  border-radius: 9px;
  color: rgba(0,245,212,.88);
  background: rgba(0,245,212,.065);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 11px;
  font-weight: 860;
  cursor: pointer;
}
.hotkey-capture:focus {
  outline: 0;
  border-color: rgba(244,210,138,.48);
  color: #fff0c7;
  background: rgba(244,210,138,.10);
}
.hotkey-row span {
  color: var(--music-stage-muted);
  font-size: 12px;
}
.beat-modal {
  width: min(520px, calc(100vw - 42px));
}
.beat-modal-copy {
  margin: 0;
  padding: 14px 18px 4px;
  color: rgba(255,255,255,.52);
  font-size: 12px;
  line-height: 1.65;
}
.beat-modal-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  padding: 14px 18px;
}
.beat-modal-grid button {
  min-height: 112px;
  padding: 14px;
  border: 1px solid rgba(255,255,255,.09);
  border-radius: 16px;
  background: rgba(255,255,255,.045);
  color: rgba(255,255,255,.86);
  text-align: left;
  cursor: pointer;
}
.beat-modal-grid button:hover:not(:disabled) {
  border-color: rgba(0,245,212,.32);
  background: rgba(0,245,212,.08);
}
.beat-modal-grid button:disabled {
  cursor: not-allowed;
  opacity: .52;
}
.beat-modal-grid b {
  display: block;
  margin-bottom: 7px;
  color: #eafffb;
  font-size: 14px;
}
.beat-modal-grid span {
  display: block;
  color: rgba(255,255,255,.48);
  font-size: 11px;
  line-height: 1.55;
}
.beat-modal-status {
  margin: 0 18px 18px;
  padding: 9px 10px;
  border: 1px solid rgba(244,210,138,.18);
  border-radius: 12px;
  color: rgba(244,210,138,.82);
  background: rgba(244,210,138,.055);
  font-size: 11px;
  font-weight: 760;
}
.collect-modal {
  width: min(430px, calc(100vw - 42px));
}
.collect-modal-head {
  display: flex;
  justify-content: space-between;
  gap: 14px;
  padding: 18px;
  border-bottom: 1px solid rgba(255,255,255,.08);
}
.collect-modal-head span {
  display: block;
  color: rgba(255,255,255,.88);
  font-size: 12px;
  font-weight: 860;
  letter-spacing: .14em;
}
.collect-modal-head small {
  display: block;
  margin-top: 6px;
  color: rgba(255,255,255,.45);
  font-size: 11px;
}
.collect-modal-head button {
  width: 30px;
  height: 30px;
  border: 1px solid rgba(255,255,255,.1);
  border-radius: 50%;
  color: rgba(255,255,255,.62);
  background: rgba(255,255,255,.045);
  cursor: pointer;
}
.collect-new-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
  padding: 14px 18px 8px;
}
.collect-new-row input {
  min-width: 0;
  height: 34px;
  padding: 0 11px;
  border: 1px solid rgba(255,255,255,.1);
  border-radius: 12px;
  color: rgba(255,255,255,.84);
  background: rgba(0,0,0,.20);
  outline: none;
}
.collect-new-row button,
.collect-list button {
  border: 1px solid rgba(0,245,212,.2);
  border-radius: 12px;
  color: rgba(234,255,251,.9);
  background: rgba(0,245,212,.075);
  cursor: pointer;
}
.collect-new-row button {
  padding: 0 12px;
  font-size: 12px;
  font-weight: 760;
}
.collect-list {
  display: grid;
  gap: 8px;
  max-height: 280px;
  overflow: auto;
  padding: 10px 18px 18px;
}
.collect-list button {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 40px;
  padding: 0 12px;
  text-align: left;
}
.collect-list button span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.collect-list button small {
  color: rgba(244,210,138,.72);
}
.collect-empty {
  padding: 14px;
  border: 1px dashed rgba(255,255,255,.12);
  border-radius: 14px;
  color: rgba(255,255,255,.42);
  text-align: center;
  font-size: 12px;
}
.gesture-hud {
  position: fixed;
  left: 50%;
  bottom: 116px;
  z-index: 31;
  display: grid;
  gap: 4px;
  width: min(440px, calc(100vw - 44px));
  padding: 12px 16px;
  border: 1px solid rgba(255,255,255,.10);
  border-radius: 16px;
  color: rgba(255,255,255,.78);
  background: rgba(10,10,16,.42);
  box-shadow: 0 18px 60px rgba(0,0,0,.4);
  backdrop-filter: blur(28px) saturate(1.4);
  pointer-events: none;
  opacity: 0;
  transform: translate(-50%, 8px);
  transition: all .2s;
}
.gesture-hud.show {
  opacity: 1;
  transform: translate(-50%, 0);
}
.gesture-hud b {
  color: var(--app-mineradio-champagne, #f4d28a);
  font-size: 11px;
  letter-spacing: .16em;
}
.gesture-hud span {
  font-size: 12px;
  line-height: 1.45;
}
.visual-guide-list {
  display: grid;
  gap: 10px;
  padding: 16px;
}
.visual-guide-list.staged {
  padding-bottom: 8px;
}
.visual-guide-item {
  display: grid;
  grid-template-columns: 34px minmax(0, 1fr);
  gap: 12px;
  padding: 12px;
  border: 1px solid rgba(255,255,255,.08);
  border-radius: 15px;
  background: rgba(255,255,255,.04);
}
.visual-guide-item b {
  color: rgba(0,245,212,.82);
  font-size: 12px;
  font-weight: 900;
}
.visual-guide-item strong,
.visual-guide-item small {
  display: block;
}
.visual-guide-item strong {
  margin-bottom: 4px;
  font-size: 13px;
}
.visual-guide-item small {
  color: var(--music-stage-muted);
  font-size: 12px;
  line-height: 1.45;
}
.visual-guide-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 0 16px 16px;
}
.visual-guide-actions button {
  height: 30px;
  padding: 0 12px;
  border: 1px solid rgba(255,255,255,.10);
  border-radius: 999px;
  color: rgba(255,255,255,.68);
  background: rgba(255,255,255,.045);
  font-size: 11px;
  font-weight: 780;
  cursor: pointer;
}
.visual-guide-actions button:not(:disabled):hover {
  color: #fff;
  border-color: rgba(0,245,212,.30);
  background: rgba(0,245,212,.085);
}
.visual-guide-actions button:disabled {
  opacity: .38;
  cursor: default;
}
.music-fx-seg {
  display: grid;
  grid-auto-flow: column;
  grid-auto-columns: 1fr;
  gap: 6px;
  margin: 8px 0;
  padding: 4px;
  border: 1px solid rgba(255,255,255,.075);
  border-radius: 14px;
  background: rgba(255,255,255,.035);
}
.music-fx-seg button {
  height: 30px;
  border: 0;
  border-radius: 10px;
  color: var(--music-stage-muted);
  background: transparent;
  font-size: 11px;
  font-weight: 760;
  cursor: pointer;
  transition: color .18s, background .18s, box-shadow .18s;
}
.music-fx-seg button:hover,
.music-fx-seg button.on {
  color: var(--music-stage-text);
  background: rgba(0,245,212,.10);
  box-shadow: inset 0 1px 0 rgba(255,255,255,.12);
}
.music-fx-stepper {
  display: grid;
  grid-template-columns: 34px minmax(0, 1fr) 34px 58px;
  align-items: center;
  gap: 6px;
  margin: 8px 0 10px;
  padding: 4px;
  border: 1px solid rgba(255,255,255,.075);
  border-radius: 14px;
  background: rgba(255,255,255,.035);
}
.music-fx-stepper span {
  min-width: 0;
  color: var(--music-stage-text);
  font-size: 12px;
  font-weight: 720;
  text-align: center;
}
.music-fx-stepper b {
  color: #00f5d4;
  font-variant-numeric: tabular-nums;
}
.music-fx-stepper button {
  height: 30px;
  border: 0;
  border-radius: 10px;
  color: var(--music-stage-muted);
  background: transparent;
  font-size: 11px;
  font-weight: 780;
  cursor: pointer;
  transition: color .18s, background .18s, box-shadow .18s;
}
.music-fx-stepper button:hover {
  color: var(--music-stage-text);
  background: rgba(0,245,212,.10);
  box-shadow: inset 0 1px 0 rgba(255,255,255,.12);
}
.music-fx-slider {
  display: block;
  padding: 10px 0;
  border-top: 1px solid rgba(255,255,255,.075);
}
.music-fx-slider span {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 8px;
  color: var(--music-stage-text);
  font-size: 13px;
  font-weight: 680;
}
.music-fx-slider b {
  color: var(--music-stage-muted);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.music-fx-slider input {
  width: 100%;
  accent-color: #00f5d4;
}
.music-detail-mask {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  background: rgba(0,0,0,.58);
  backdrop-filter: blur(12px);
}
.music-detail-modal {
  width: min(620px, calc(100vw - 36px));
  max-height: calc(100vh - 42px);
  overflow: auto;
  padding: 20px;
  border: 1px solid rgba(0,245,212,.16);
  border-radius: 20px;
  color: var(--music-stage-text);
  background: linear-gradient(145deg, rgba(14,16,20,.92), rgba(5,6,8,.96));
  box-shadow: 0 28px 90px rgba(0,0,0,.48), inset 0 1px 0 rgba(255,255,255,.08);
}
.music-detail-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 14px;
}
.music-detail-title {
  max-width: 480px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 18px;
  font-weight: 820;
}
.music-detail-sub {
  margin-top: 4px;
  color: var(--music-stage-muted);
  font-size: 12px;
}
.music-detail-close {
  width: 32px;
  height: 32px;
  border: 1px solid rgba(255,255,255,.11);
  border-radius: 10px;
  color: var(--music-stage-muted);
  background: rgba(255,255,255,.045);
  cursor: pointer;
}
.music-detail-close:hover {
  color: var(--music-stage-text);
  background: rgba(255,255,255,.09);
}
.music-detail-hero {
  display: grid;
  grid-template-columns: 130px minmax(0, 1fr);
  gap: 16px;
  align-items: start;
}
.music-detail-hero img,
.music-detail-cover-empty {
  width: 130px;
  height: 130px;
  border-radius: 16px;
  object-fit: cover;
  background: rgba(255,255,255,.06);
  box-shadow: 0 18px 46px rgba(0,0,0,.28), inset 0 1px 0 rgba(255,255,255,.12);
}
.music-detail-cover-empty {
  display: grid;
  place-items: center;
  color: rgba(255,255,255,.34);
  font-size: 42px;
}
.music-detail-grid {
  display: grid;
  grid-template-columns: 72px minmax(0, 1fr);
  gap: 9px 12px;
  font-size: 12px;
}
.music-detail-grid span {
  color: var(--music-stage-muted);
}
.music-detail-grid b {
  min-width: 0;
  overflow: hidden;
  color: var(--music-stage-text);
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
}
body:not([arco-theme='dark']) .mineradio-player {
  .music-fx-panel {
    border-color: rgba(255,255,255,.58);
    background: rgba(242,248,250,.62);
    box-shadow: 0 24px 72px rgba(55,78,94,.20), inset 0 1px 0 rgba(255,255,255,.74);
  }
  .music-detail-modal {
    border-color: rgba(255,255,255,.58);
    background: rgba(242,248,250,.82);
    box-shadow: 0 24px 72px rgba(55,78,94,.20), inset 0 1px 0 rgba(255,255,255,.74);
  }
  .music-fx-reset,
  .music-fx-action,
  .music-fx-toggle,
  .music-fx-slot button,
  .music-fx-color-row label,
  .music-fx-color-row input[type='color'],
  .music-fx-preset,
  .music-fx-seg,
  .music-fx-slider {
    border-color: rgba(20,32,42,.10);
  }
}
@media (max-height: 760px) {
  .music-fx-panel {
    top: 56px;
    bottom: 116px;
    width: min(410px, calc(100vw - 48px));
    padding: 14px;
  }
}
@media (max-height: 520px) {
  .mineradio-titlebar {
    height: 30px;
  }
  .music-fx-panel {
    top: 34px;
    bottom: 64px;
    right: -360px;
    width: min(340px, calc(100vw - 36px));
    padding: 12px;
  }
  .music-fx-panel.show {
    right: 12px;
  }
  .music-beat-chip {
    top: 34px;
    right: 12px;
    min-width: 76px;
    padding: 7px 10px;
  }
}
@media (max-width: 980px) {
  .music-fx-panel {
    left: 18px;
    right: 18px;
    width: auto;
    top: 62px;
    bottom: 132px;
  }
  .music-fx-panel.show {
    right: 18px;
  }
}
.mineradio-titlebar {
  position: absolute;
  z-index: 20;
  top: 0;
  left: 0;
  right: 0;
  height: 42px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 16px;
}
.win-dot {
  width: 13px;
  height: 13px;
  border: 0;
  border-radius: 50%;
  cursor: pointer;
}
.win-dot.close { background: #ff5f57; }
.win-dot.min { background: #febc2e; }
.win-dot.max { background: #28c840; }
</style>
