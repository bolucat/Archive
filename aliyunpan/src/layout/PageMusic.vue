<script setup lang='ts'>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { SkipBack, SkipForward, Pause, Play, FileText, Music, Search, X, RotateCw, List, Maximize2, Minus } from 'lucide-vue-next'
import { KeyboardState, useAppStore, useKeyboardStore } from '../store'
import message from '../utils/message'
import { TestAlt, TestKey, TestShift } from '../utils/keyboardhelper'
import { getRawUrl } from '../utils/proxyhelper'
import { IPageMusicTrack } from '../store/appstore'
import { fetchMusicMetadata, findActiveLineIndex, LyricLine, MusicMetadata } from '../utils/musicMetadata'

const appStore = useAppStore()
const keyboardStore = useKeyboardStore()

keyboardStore.$subscribe((_m: any, state: KeyboardState) => {
  if (TestAlt('f4', state.KeyDownEvent, handleHideClick)) return
  if (TestAlt('m', state.KeyDownEvent, handleMinClick)) return
  if (TestAlt('enter', state.KeyDownEvent, handleMaxClick)) return
  if (TestKey('f11', state.KeyDownEvent, handleMaxClick)) return
  if (TestKey(' ', state.KeyDownEvent, togglePlay)) return
  if (TestShift('arrowright', state.KeyDownEvent, () => playNext(false))) return
  if (TestShift('arrowleft', state.KeyDownEvent, playPrev)) return
  if (TestKey('arrowright', state.KeyDownEvent, () => seekRelative(5))) return
  if (TestKey('arrowleft', state.KeyDownEvent, () => seekRelative(-5))) return
  if (TestKey('arrowup', state.KeyDownEvent, () => setVolume(Math.min(1, volume.value + 0.05)))) return
  if (TestKey('arrowdown', state.KeyDownEvent, () => setVolume(Math.max(0, volume.value - 0.05)))) return
})

const onKeyDown = (event: KeyboardEvent) => {
  const ele = (event.srcElement || event.target) as any
  const nodeName = ele && ele.nodeName
  if (document.body.getElementsByClassName('arco-modal-container').length) return
  if (event.key == 'Control' || event.key == 'Shift' || event.key == 'Alt' || event.key == 'Meta') return
  const isInput = nodeName == 'INPUT' || nodeName == 'TEXTAREA' || false
  if (!isInput) {
    keyboardStore.KeyDown(event)
  }
}

type PlayMode = 'list' | 'loop-list' | 'loop-one' | 'shuffle'

const audioRef = ref<HTMLAudioElement | null>(null)
const playlist = ref<IPageMusicTrack[]>([])
const currentIndex = ref(0)
const isPlaying = ref(false)
const isLoading = ref(false)
const currentTime = ref(0)
const duration = ref(0)
const volume = ref(0.8)
const muted = ref(false)
const playMode = ref<PlayMode>('loop-list')
const isTop = ref(false)
const showPlaylist = ref(true)
const showLyrics = ref(true)
const errorMsg = ref('')
const seekingPreviewTime = ref<number | null>(null)
const shuffleHistory = ref<number[]>([])
const VOL_KEY = 'pageMusic.volume'
const MODE_KEY = 'pageMusic.mode'
const LYRIC_KEY = 'pageMusic.showLyrics'
const PLAYLIST_KEY = 'pageMusic.showPlaylist'
const TAB_KEY = 'pageMusic.tab'
const FAV_KEY = 'pageMusic.favorites'
const RECENT_KEY = 'pageMusic.recents'
const RECENT_MAX = 50

type MusicTab = 'now' | 'queue' | 'fav' | 'recent'
const activeTab = ref<MusicTab>('now')
const favorites = ref<IPageMusicTrack[]>([])
const recents = ref<IPageMusicTrack[]>([])
const listFilter = ref('')
const sleepEndAt = ref<number | null>(null)
const sleepEndOfTrack = ref(false)
const sleepRemaining = ref(0)
const sleepPopVisible = ref(false)
let sleepTickTimer: number | null = null

const meta = ref<MusicMetadata | null>(null)
const metaLoading = ref(false)
const lyricListRef = ref<HTMLElement | null>(null)
let metaToken = 0

const lyricLines = computed<LyricLine[]>(() => meta.value?.lines || [])
const hasLyrics = computed(() => lyricLines.value.length > 0)
const activeLineIndex = computed(() => findActiveLineIndex(lyricLines.value, currentTime.value))

const coverUrl = computed(() => {
  return currentTrack.value?.thumbnail || meta.value?.cover || ''
})
const displayArtist = computed(() => meta.value?.artist || '')
const displayAlbum = computed(() => meta.value?.album || '')

function favKey(t: { user_id?: string, drive_id?: string, file_id?: string } | undefined | null): string {
  if (!t) return ''
  return `${t.user_id || ''}|${t.drive_id || ''}|${t.file_id || ''}`
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

function matchTrack(t: IPageMusicTrack, q: string): boolean {
  if (!q) return true
  const lc = q.toLowerCase()
  return (t.file_name || '').toLowerCase().includes(lc)
}

const favoriteSet = computed(() => new Set(favorites.value.map(favKey)))
const isFavorited = computed(() => {
  const t = currentTrack.value
  if (!t) return false
  return favoriteSet.value.has(favKey(t))
})

const filteredQueue = computed(() => playlist.value.filter(t => matchTrack(t, listFilter.value)))
const filteredFavorites = computed(() => favorites.value.filter(t => matchTrack(t, listFilter.value)))
const filteredRecents = computed(() => recents.value.filter(t => matchTrack(t, listFilter.value)))

const activeListSource = computed<IPageMusicTrack[]>(() => {
  if (activeTab.value === 'queue') return filteredQueue.value
  if (activeTab.value === 'fav') return filteredFavorites.value
  if (activeTab.value === 'recent') return filteredRecents.value
  return []
})

const activeListEmptyText = computed(() => {
  if (activeTab.value === 'queue') return playlist.value.length ? '没有匹配的曲目' : '播放列表为空'
  if (activeTab.value === 'fav') return favorites.value.length ? '没有匹配的曲目' : '还没有收藏，点击底部 ❤ 收藏当前播放曲'
  if (activeTab.value === 'recent') return recents.value.length ? '没有匹配的曲目' : '最近播放为空'
  return ''
})

function isCurrentTrack(t: IPageMusicTrack): boolean {
  const cur = currentTrack.value
  if (!cur) return false
  return favKey(cur) === favKey(t)
}

function persistFavorites() { saveJson(FAV_KEY, favorites.value) }
function persistRecents() { saveJson(RECENT_KEY, recents.value) }

function toggleFavorite(track: IPageMusicTrack | undefined | null) {
  if (!track) return
  const key = favKey(track)
  const idx = favorites.value.findIndex(f => favKey(f) === key)
  if (idx >= 0) {
    favorites.value.splice(idx, 1)
  } else {
    favorites.value.unshift({ ...track })
  }
  persistFavorites()
}

function removeFavorite(track: IPageMusicTrack) {
  const key = favKey(track)
  const idx = favorites.value.findIndex(f => favKey(f) === key)
  if (idx >= 0) {
    favorites.value.splice(idx, 1)
    persistFavorites()
  }
}

function pushRecent(track: IPageMusicTrack | undefined | null) {
  if (!track) return
  const key = favKey(track)
  const idx = recents.value.findIndex(r => favKey(r) === key)
  if (idx >= 0) recents.value.splice(idx, 1)
  recents.value.unshift({ ...track })
  if (recents.value.length > RECENT_MAX) recents.value.length = RECENT_MAX
  persistRecents()
}

function clearRecents() {
  recents.value = []
  persistRecents()
}

function playFromTrack(track: IPageMusicTrack) {
  const key = favKey(track)
  let idx = playlist.value.findIndex(t => favKey(t) === key)
  if (idx < 0) {
    playlist.value.push({ ...track })
    idx = playlist.value.length - 1
  }
  activeTab.value = 'now'
  loadIndex(idx, true)
}

function clearSleepTimerInternal() {
  if (sleepTickTimer !== null) {
    window.clearInterval(sleepTickTimer)
    sleepTickTimer = null
  }
  sleepEndAt.value = null
  sleepRemaining.value = 0
  sleepEndOfTrack.value = false
}

function startSleepTick() {
  if (sleepTickTimer !== null) window.clearInterval(sleepTickTimer)
  sleepTickTimer = window.setInterval(() => {
    if (sleepEndAt.value === null) {
      if (sleepTickTimer !== null) {
        window.clearInterval(sleepTickTimer)
        sleepTickTimer = null
      }
      return
    }
    const remain = Math.max(0, Math.ceil((sleepEndAt.value - Date.now()) / 1000))
    sleepRemaining.value = remain
    if (remain <= 0) {
      try { audioRef.value?.pause() } catch {}
      isPlaying.value = false
      clearSleepTimerInternal()
    }
  }, 1000)
}

function setSleepTimer(opt: number | 'end-of-track' | 0) {
  clearSleepTimerInternal()
  sleepPopVisible.value = false
  if (opt === 0) return
  if (opt === 'end-of-track') {
    sleepEndOfTrack.value = true
    return
  }
  sleepEndAt.value = Date.now() + opt * 60 * 1000
  sleepRemaining.value = opt * 60
  startSleepTick()
}

const sleepActive = computed(() => sleepEndAt.value !== null || sleepEndOfTrack.value)
const sleepLabel = computed(() => {
  if (sleepEndOfTrack.value) return '播完即停'
  if (sleepEndAt.value === null) return ''
  return formatTime(sleepRemaining.value)
})

const currentTrack = computed<IPageMusicTrack | undefined>(() => playlist.value[currentIndex.value])
const trackTitle = computed(() => {
  const name = currentTrack.value?.file_name || ''
  return stripExt(name)
})
const trackExt = computed(() => {
  const t = currentTrack.value
  if (!t) return ''
  if (t.ext) return t.ext.replace(/^\./, '').toUpperCase()
  const dot = (t.file_name || '').lastIndexOf('.')
  if (dot < 0) return ''
  return (t.file_name || '').slice(dot + 1).toUpperCase()
})

function stripExt(name: string): string {
  if (!name) return ''
  const i = name.lastIndexOf('.')
  if (i <= 0) return name
  return name.slice(0, i)
}

function formatTime(sec: number): string {
  if (!isFinite(sec) || sec < 0) sec = 0
  const total = Math.floor(sec)
  const h = Math.floor(total / 3600)
  const m = Math.floor((total % 3600) / 60)
  const s = total % 60
  const pad = (n: number) => n.toString().padStart(2, '0')
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`
}

const progressPercent = computed(() => {
  if (!duration.value) return 0
  const t = seekingPreviewTime.value ?? currentTime.value
  return Math.max(0, Math.min(100, (t / duration.value) * 100))
})

const displayTime = computed(() => formatTime(seekingPreviewTime.value ?? currentTime.value))

async function resolveUrlForIndex(idx: number): Promise<string> {
  const track = playlist.value[idx]
  if (!track) return ''
  const data = await getRawUrl(
    track.user_id,
    track.drive_id,
    track.file_id,
    track.encType || '',
    track.password || '',
    false,
    'audio'
  )
  if (typeof data === 'string') {
    throw new Error(data || '获取音频地址失败')
  }
  return data.url || ''
}

async function loadIndex(idx: number, autoplay = true) {
  if (!playlist.value.length) return
  if (idx < 0) idx = 0
  if (idx >= playlist.value.length) idx = playlist.value.length - 1
  currentIndex.value = idx
  errorMsg.value = ''
  duration.value = 0
  currentTime.value = 0
  const audio = audioRef.value
  if (!audio) return
  try {
    isLoading.value = true
    const url = await resolveUrlForIndex(idx)
    if (!url) throw new Error('未获取到播放地址')
    audio.src = url
    audio.load()
    if (autoplay) {
      try {
        await audio.play()
        isPlaying.value = true
      } catch (e) {
        isPlaying.value = false
      }
    }
  } catch (e: any) {
    errorMsg.value = e?.message || String(e)
    message.error('加载失败: ' + errorMsg.value)
    isPlaying.value = false
  } finally {
    isLoading.value = false
  }
  loadMetaForCurrent()
  pushRecent(currentTrack.value)
}

async function loadMetaForCurrent() {
  const t = currentTrack.value
  metaToken++
  const my = metaToken
  if (!t) {
    meta.value = null
    return
  }
  metaLoading.value = true
  try {
    const data = await fetchMusicMetadata({
      filename: t.file_name,
      durationSec: duration.value || undefined
    })
    if (my !== metaToken) return
    meta.value = data
    lastScrollLineIndex.value = -1
    nextTick(() => scrollToActiveLine(true))
  } catch (e) {
    if (my === metaToken) meta.value = null
  } finally {
    if (my === metaToken) metaLoading.value = false
  }
}

const lastScrollLineIndex = ref(-1)
function scrollToActiveLine(force = false) {
  if (!showLyrics.value) return
  const list = lyricListRef.value
  if (!list) return
  const idx = activeLineIndex.value
  if (!force && idx === lastScrollLineIndex.value) return
  lastScrollLineIndex.value = idx
  const target = list.querySelector<HTMLElement>(`[data-li="${idx}"]`)
  if (!target) {
    list.scrollTop = 0
    return
  }
  const containerH = list.clientHeight
  const top = target.offsetTop - containerH / 2 + target.clientHeight / 2
  list.scrollTo({ top: Math.max(0, top), behavior: force ? 'auto' : 'smooth' })
}

function togglePlay() {
  const audio = audioRef.value
  if (!audio) return
  if (!audio.src) {
    loadIndex(currentIndex.value, true)
    return
  }
  if (audio.paused) {
    audio.play().then(() => { isPlaying.value = true }).catch(() => { isPlaying.value = false })
  } else {
    audio.pause()
    isPlaying.value = false
  }
}

function playPrev() {
  if (!playlist.value.length) return
  if (playMode.value === 'shuffle') {
    if (shuffleHistory.value.length > 1) {
      shuffleHistory.value.pop()
      const prev = shuffleHistory.value[shuffleHistory.value.length - 1]
      loadIndex(prev, true)
      return
    }
    loadIndex(pickShuffleIndex(), true)
    return
  }
  let idx = currentIndex.value - 1
  if (idx < 0) idx = playlist.value.length - 1
  loadIndex(idx, true)
}

function playNext(auto = false) {
  if (!playlist.value.length) return
  if (playMode.value === 'loop-one' && auto) {
    loadIndex(currentIndex.value, true)
    return
  }
  if (playMode.value === 'shuffle') {
    const next = pickShuffleIndex()
    loadIndex(next, true)
    return
  }
  let idx = currentIndex.value + 1
  if (idx >= playlist.value.length) {
    if (playMode.value === 'list' && auto) {
      isPlaying.value = false
      return
    }
    idx = 0
  }
  loadIndex(idx, true)
}

function pickShuffleIndex(): number {
  const total = playlist.value.length
  if (total <= 1) return 0
  let next = currentIndex.value
  let attempts = 0
  while (next === currentIndex.value && attempts < 32) {
    next = Math.floor(Math.random() * total)
    attempts++
  }
  shuffleHistory.value.push(next)
  if (shuffleHistory.value.length > 100) shuffleHistory.value.shift()
  return next
}

function selectTrack(idx: number) {
  if (idx === currentIndex.value) {
    togglePlay()
  } else {
    loadIndex(idx, true)
  }
}

function seekRelative(delta: number) {
  const audio = audioRef.value
  if (!audio || !duration.value) return
  audio.currentTime = Math.max(0, Math.min(duration.value, (audio.currentTime || 0) + delta))
}

function onProgressMouseDown(ev: MouseEvent) {
  const bar = ev.currentTarget as HTMLElement
  if (!bar || !duration.value) return
  const rect = bar.getBoundingClientRect()
  const update = (clientX: number) => {
    const x = Math.max(0, Math.min(rect.width, clientX - rect.left))
    seekingPreviewTime.value = (x / rect.width) * duration.value
  }
  update(ev.clientX)
  const onMove = (e: MouseEvent) => update(e.clientX)
  const onUp = (e: MouseEvent) => {
    update(e.clientX)
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
    if (audioRef.value && seekingPreviewTime.value !== null) {
      audioRef.value.currentTime = seekingPreviewTime.value
    }
    seekingPreviewTime.value = null
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

function setVolume(v: number) {
  volume.value = Math.max(0, Math.min(1, v))
  if (audioRef.value) {
    audioRef.value.volume = volume.value
    if (volume.value > 0 && muted.value) {
      muted.value = false
      audioRef.value.muted = false
    }
  }
  try { localStorage.setItem(VOL_KEY, String(volume.value)) } catch {}
}

function onVolumeBarMouseDown(ev: MouseEvent) {
  const bar = ev.currentTarget as HTMLElement
  if (!bar) return
  const rect = bar.getBoundingClientRect()
  const update = (clientX: number) => {
    const x = Math.max(0, Math.min(rect.width, clientX - rect.left))
    setVolume(x / rect.width)
  }
  update(ev.clientX)
  const onMove = (e: MouseEvent) => update(e.clientX)
  const onUp = () => {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

function toggleMute() {
  if (!audioRef.value) return
  muted.value = !muted.value
  audioRef.value.muted = muted.value
}

function cyclePlayMode() {
  const order: PlayMode[] = ['loop-list', 'loop-one', 'shuffle', 'list']
  const i = order.indexOf(playMode.value)
  playMode.value = order[(i + 1) % order.length]
  shuffleHistory.value = []
  try { localStorage.setItem(MODE_KEY, playMode.value) } catch {}
}

const playModeIcon = computed(() => {
  switch (playMode.value) {
    case 'list': return 'iconpaixu1'
    case 'loop-list': return 'iconreload-1-icon'
    case 'loop-one': return 'iconreload-1-icon'
    case 'shuffle': return 'iconrss_video'
    default: return 'iconreload-1-icon'
  }
})

const playModeText = computed(() => {
  switch (playMode.value) {
    case 'list': return '顺序播放'
    case 'loop-list': return '列表循环'
    case 'loop-one': return '单曲循环'
    case 'shuffle': return '随机播放'
    default: return ''
  }
})

const handleHideClick = () => {
  try { audioRef.value?.pause() } catch {}
  if (window.WebToWindow) window.WebToWindow({ cmd: 'close' })
  else window.close()
}
const handleMinClick = () => {
  if (window.WebToWindow) window.WebToWindow({ cmd: 'minsize' })
}
const handleMaxClick = () => {
  if (window.WebToWindow) window.WebToWindow({ cmd: 'maxsize' })
}
const handleTop = () => {
  if (window.WebToWindow) window.WebToWindow({ cmd: 'top' }, (res: string) => {
    isTop.value = res === 'top'
  })
}

function onAudioLoaded() {
  duration.value = audioRef.value?.duration || 0
}
function onAudioTimeUpdate() {
  currentTime.value = audioRef.value?.currentTime || 0
}
function onAudioEnded() {
  if (sleepEndOfTrack.value) {
    sleepEndOfTrack.value = false
    isPlaying.value = false
    try { audioRef.value?.pause() } catch {}
    return
  }
  playNext(true)
}
function onAudioError() {
  if (!audioRef.value?.src) return
  errorMsg.value = '播放出错'
  isPlaying.value = false
}
function onAudioPlay() { isPlaying.value = true }
function onAudioPause() { isPlaying.value = false }

onMounted(() => {
  window.addEventListener('keydown', onKeyDown, true)
  try {
    const v = parseFloat(localStorage.getItem(VOL_KEY) || '')
    if (!isNaN(v) && v >= 0 && v <= 1) volume.value = v
    const m = localStorage.getItem(MODE_KEY) as PlayMode | null
    if (m && ['list', 'loop-list', 'loop-one', 'shuffle'].includes(m)) playMode.value = m
    const ly = localStorage.getItem(LYRIC_KEY)
    if (ly === '0') showLyrics.value = false
    const pl = localStorage.getItem(PLAYLIST_KEY)
    if (pl === '0') showPlaylist.value = false
    const tab = localStorage.getItem(TAB_KEY)
    if (tab === 'now' || tab === 'queue' || tab === 'fav' || tab === 'recent') {
      activeTab.value = tab
    }
  } catch {}
  favorites.value = (loadJson<IPageMusicTrack[]>(FAV_KEY, []) || []).filter(t => t && t.file_id)
  recents.value = (loadJson<IPageMusicTrack[]>(RECENT_KEY, []) || []).filter(t => t && t.file_id)

  const data = appStore.pageMusic
  if (!data) {
    message.error('未提供音乐播放参数')
    return
  }
  document.title = data.file_name || '音乐播放器'
  setTimeout(() => { document.title = data.file_name || '音乐播放器' }, 1000)

  playlist.value = (data.playlist || []).slice()
  if (!playlist.value.length) {
    playlist.value = [{
      user_id: data.user_id,
      drive_id: data.drive_id,
      file_id: data.file_id,
      parent_file_id: data.parent_file_id,
      file_name: data.file_name,
      encType: data.encType,
      password: data.password
    }]
  }

  let idx = playlist.value.findIndex(t => t.file_id === data.file_id)
  if (idx < 0) idx = 0
  currentIndex.value = idx

  if (audioRef.value) {
    audioRef.value.volume = volume.value
  }
  loadIndex(idx, true)
})

watch(currentTrack, (t) => {
  if (t) document.title = t.file_name
})

watch(activeLineIndex, () => {
  scrollToActiveLine(false)
})

watch(showLyrics, (v) => {
  try { localStorage.setItem(LYRIC_KEY, v ? '1' : '0') } catch {}
  if (v) nextTick(() => scrollToActiveLine(true))
})

watch(showPlaylist, (v) => {
  try { localStorage.setItem(PLAYLIST_KEY, v ? '1' : '0') } catch {}
})

watch(activeTab, (v) => {
  try { localStorage.setItem(TAB_KEY, v) } catch {}
  listFilter.value = ''
  if (v === 'now') nextTick(() => scrollToActiveLine(true))
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown, true)
  if (sleepTickTimer !== null) {
    window.clearInterval(sleepTickTimer)
    sleepTickTimer = null
  }
  try { audioRef.value?.pause() } catch {}
})
</script>

<template>
  <a-layout style='height: 100vh' draggable='false' class='pageMusic'>
    <transition name='bgFade'>
      <div :key='coverUrl || "_none"' class='musicBg' :style="coverUrl ? { backgroundImage: `url(${coverUrl})` } : {}"></div>
    </transition>
    <div class='musicBgOverlay'></div>

    <a-layout-header id='xbyhead' draggable='false' class='musicHeader'>
      <div id='xbyhead2' class='q-electron-drag'>
        <a-button type='text' tabindex='-1'>
          <Music :size='16' :stroke-width='1.8' />
        </a-button>
        <div class='title'>{{ currentTrack?.file_name || '音乐播放器' }}</div>
        <div class='flexauto'></div>
        <a-button type='text' tabindex='-1' :title="(isTop ? '取消置顶' : '置顶')" @click='handleTop'>
          <IconFont :name="(isTop ? 'iconquxiaozhiding' : 'iconzhiding')" />
        </a-button>
        <a-button type='text' tabindex='-1' title='最小化 Alt+M' @click='handleMinClick'>
          <Minus :size='16' :stroke-width='1.8' />
        </a-button>
        <a-button type='text' tabindex='-1' title='最大化 Alt+Enter' @click='handleMaxClick'>
          <Maximize2 :size='14' :stroke-width='1.8' />
        </a-button>
        <a-button type='text' tabindex='-1' title='关闭 Alt+F4' @click='handleHideClick'>
          <X :size='16' :stroke-width='1.8' />
        </a-button>
      </div>
    </a-layout-header>

    <div class='musicTabs'>
      <div class='musicTabsLeft'>
        <button :class="{ musicTabBtn: 1, active: activeTab === 'now' }" @click="activeTab = 'now'">
          <Music :size='16' :stroke-width='1.8' class='musicTabIcon' />
          <span>正在播放</span>
        </button>
        <button :class="{ musicTabBtn: 1, active: activeTab === 'queue' }" @click="activeTab = 'queue'">
          <List :size='16' :stroke-width='1.8' class='musicTabIcon' />
          <span>播放列表</span>
          <span class='musicTabBadge'>{{ playlist.length }}</span>
        </button>
        <button :class="{ musicTabBtn: 1, active: activeTab === 'fav' }" @click="activeTab = 'fav'">
          <svg class='musicTabIcon' viewBox='0 0 1024 1024'>
            <path d='M512 891.6L138 565.4C73.5 510 64 410.7 117.7 343.5c47.7-59.6 134.4-71.5 195.7-26.7l198.6 145 198.6-145c61.3-44.8 148-32.9 195.7 26.7C960 410.7 950.5 510 886 565.4L512 891.6z' />
          </svg>
          <span>我的收藏</span>
          <span class='musicTabBadge'>{{ favorites.length }}</span>
        </button>
        <button :class="{ musicTabBtn: 1, active: activeTab === 'recent' }" @click="activeTab = 'recent'">
          <RotateCw :size='16' :stroke-width='1.8' class='musicTabIcon' />
          <span>最近播放</span>
        </button>
      </div>
      <div v-if="activeTab !== 'now'" class='musicTabsRight'>
        <div class='musicSearch'>
          <Search :size='14' :stroke-width='1.8' class='musicSearchIcon' />
          <input v-model='listFilter' placeholder='搜索曲名…' class='musicSearchInput' />
          <button v-if='listFilter' class='musicSearchClear' @click="listFilter = ''" title='清空'>
            <X :size='14' :stroke-width='2' />
          </button>
        </div>
        <a-button v-if="activeTab === 'recent' && recents.length"
                  type='text' size='mini' class='musicListAction' @click='clearRecents'>
          清空历史
        </a-button>
      </div>
    </div>

    <a-layout-content class='musicMainArea'>
      <transition name='musicFade' mode='out-in'>
        <div v-if="activeTab === 'now'" key='now' class='musicNow'>
          <div class='musicNowLeft'>
            <div :class="['musicVinyl', isPlaying ? 'playing' : '']">
              <div class='musicVinylRing'></div>
              <div class='musicVinylDisc'>
                <img v-if='coverUrl' :src='coverUrl'
                     @error="(e:any)=>{ e.currentTarget.style.display='none' }" />
                <div v-else class='musicCoverFallback'>
                  <IconFont name="iconfile-audio" />
                </div>
              </div>
              <div class='musicVinylCenter'></div>
            </div>
            <div class='musicNowMeta'>
              <div class='musicNowTitle' :title='currentTrack?.file_name'>
                {{ trackTitle || '暂无音乐' }}
              </div>
              <div v-if='displayArtist || displayAlbum' class='musicNowArtist'>
                <span v-if='displayArtist'>{{ displayArtist }}</span>
                <span v-if='displayArtist && displayAlbum' class='musicArtistDot'>·</span>
                <span v-if='displayAlbum' :title='displayAlbum'>{{ displayAlbum }}</span>
              </div>
              <div class='musicNowSub'>
                <span v-if='trackExt' class='musicNowChip'>{{ trackExt }}</span>
                <span v-if='playlist.length' class='musicNowChip'>{{ (currentIndex + 1) + ' / ' + playlist.length }}</span>
                <span v-if='errorMsg' class='musicError'>{{ errorMsg }}</span>
                <span v-else-if='isLoading' class='musicSubMuted'>音频加载中…</span>
                <span v-else-if='metaLoading' class='musicSubMuted'>正在获取歌词/封面…</span>
              </div>
            </div>
          </div>

          <div class='musicNowLyrics'>
            <div ref='lyricListRef' class='musicLyricsList'>
              <div v-if='!hasLyrics' class='musicLyricsEmpty'>
                <div>{{ metaLoading ? '正在获取歌词…' : '暂未找到歌词' }}</div>
                <div class='musicLyricsEmptyHint' v-if='!metaLoading'>
                  数据来自 LRCLIB / iTunes，建议文件命名为「歌手 - 歌名」
                </div>
              </div>
              <template v-else>
                <div class='musicLyricsSpacer'></div>
                <div
                  v-for='(line, i) in lyricLines'
                  :key='i'
                  :data-li='i'
                  :class="'musicLyricsLine' + (i === activeLineIndex ? ' active' : '') + (i < activeLineIndex ? ' past' : '')"
                  @click='audioRef && (audioRef.currentTime = line.time)'>
                  {{ line.text }}
                </div>
                <div class='musicLyricsSpacer'></div>
              </template>
            </div>
          </div>
        </div>

        <div v-else key='list' class='musicListView'>
          <div v-if='activeListSource.length' class='musicListGrid'>
            <div
              v-for='(t, i) in activeListSource'
              :key='favKey(t) + "#" + i'
              :class="['musicListRow', isCurrentTrack(t) ? 'current' : '']"
              @dblclick='playFromTrack(t)'
              @click='playFromTrack(t)'>
              <div class='musicListIdx'>
                <svg v-if='isCurrentTrack(t) && isPlaying' class='musicSidePlayIcon' viewBox='0 0 1024 1024'>
                  <path d='M276 154.7v714.6c0 25.1 27.6 40.4 49 27.1l570.7-357.3c20.1-12.6 20.1-41.7 0-54.3L325 127.6c-21.4-13.3-49 2-49 27.1z' />
                </svg>
                <span v-else>{{ i + 1 }}</span>
              </div>
              <div class='musicListThumb'>
                <img v-if='t.thumbnail' :src='t.thumbnail'
                     @error="(e:any)=>{ e.currentTarget.style.display='none' }" />
                <IconFont name="iconfile-audio" v-else />
              </div>
              <div class='musicListMeta'>
                <div class='musicListName' :title='t.file_name'>{{ stripExt(t.file_name) }}</div>
                <div class='musicListSub'>
                  <span v-if='t.ext' class='musicListExt'>{{ String(t.ext).replace(/^\./, '').toUpperCase() }}</span>
                </div>
              </div>
              <div class='musicListActions' @click.stop>
                <button class='musicIconBtn'
                        :class="{ active: favoriteSet.has(favKey(t)) }"
                        :title="favoriteSet.has(favKey(t)) ? '取消收藏' : '收藏'"
                        @click='toggleFavorite(t)'>
                  <svg viewBox='0 0 1024 1024'>
                    <path d='M512 891.6L138 565.4C73.5 510 64 410.7 117.7 343.5c47.7-59.6 134.4-71.5 195.7-26.7l198.6 145 198.6-145c61.3-44.8 148-32.9 195.7 26.7C960 410.7 950.5 510 886 565.4L512 891.6z' />
                  </svg>
                </button>
                <button v-if="activeTab === 'fav'" class='musicIconBtn' title='移除' @click='removeFavorite(t)'>
                  <IconFont name="iconclose" />
                </button>
              </div>
            </div>
          </div>
          <div v-else class='musicListEmpty'>{{ activeListEmptyText }}</div>
        </div>
      </transition>
    </a-layout-content>

    <div class='musicBottomBar'>
      <div class='musicBottomLeft'>
        <div class='musicMiniCover' :class="{ playing: isPlaying }">
          <img v-if='coverUrl' :src='coverUrl'
               @error="(e:any)=>{ e.currentTarget.style.display='none' }" />
          <div v-else class='musicMiniCoverFallback'>
            <IconFont name="iconfile-audio" />
          </div>
        </div>
        <div class='musicMiniMeta'>
          <div class='musicMiniTitle' :title='currentTrack?.file_name'>
            {{ trackTitle || '暂无音乐' }}
          </div>
          <div v-if='displayArtist' class='musicMiniArtist' :title='displayArtist'>{{ displayArtist }}</div>
        </div>
        <button class='musicIconBtn musicFavBtn'
                :class="{ active: isFavorited }"
                :disabled='!currentTrack'
                :title="isFavorited ? '取消收藏' : '收藏当前歌曲'"
                @click='toggleFavorite(currentTrack)'>
          <svg viewBox='0 0 1024 1024'>
            <path d='M512 891.6L138 565.4C73.5 510 64 410.7 117.7 343.5c47.7-59.6 134.4-71.5 195.7-26.7l198.6 145 198.6-145c61.3-44.8 148-32.9 195.7 26.7C960 410.7 950.5 510 886 565.4L512 891.6z' />
          </svg>
        </button>
      </div>

      <div class='musicBottomCenter'>
        <div class='musicBottomCtrls'>
          <a-button type='text' shape='circle' :title='playModeText' @click='cyclePlayMode'>
            <IconFont :name="playModeIcon" />
            <span class='musicModeBadge' v-if="playMode === 'loop-one'">1</span>
          </a-button>
          <a-button type='text' shape='circle' title='上一首 (Shift+Left)' @click='playPrev'>
            <SkipBack :size='18' :stroke-width='1.8' />
          </a-button>
          <a-button class='musicPlayBtn' type='primary' shape='circle' @click='togglePlay'
                    :title="isPlaying ? '暂停 (Space)' : '播放 (Space)'">
            <Pause v-if='isPlaying' :size='22' :stroke-width='2' :fill="'currentColor'" />
            <Play v-else :size='22' :stroke-width='2' :fill="'currentColor'" />
          </a-button>
          <a-button type='text' shape='circle' title='下一首 (Shift+Right)' @click='playNext(false)'>
            <SkipForward :size='18' :stroke-width='1.8' />
          </a-button>
          <a-button type='text' shape='circle' :title="showLyrics ? '隐藏歌词' : '显示歌词'"
                    :class="showLyrics ? 'musicPlaylistOn' : ''" @click='showLyrics = !showLyrics'>
            <FileText :size='18' :stroke-width='1.8' />
          </a-button>
        </div>
        <div class='musicBottomProgress'>
          <span class='musicTime'>{{ displayTime }}</span>
          <div class='musicBar' @mousedown='onProgressMouseDown'>
            <div class='musicBarFill' :style="{ width: progressPercent + '%' }"></div>
            <div class='musicBarThumb' :style="{ left: progressPercent + '%' }"></div>
          </div>
          <span class='musicTime'>{{ formatTime(duration) }}</span>
        </div>
      </div>

      <div class='musicBottomRight'>
        <div class='musicVolume'>
          <a-button type='text' shape='circle' :title="muted ? '取消静音' : '静音'" @click='toggleMute'>
            <svg v-if='muted || volume === 0' class='musicVolIcon' viewBox='0 0 1024 1024'>
              <path d='M524.8 161.6L283.2 384H128c-17.7 0-32 14.3-32 32v192c0 17.7 14.3 32 32 32h155.2l241.6 222.4c20.5 18.9 53.7 4.4 53.7-23.5V185.1c0-27.9-33.2-42.4-53.7-23.5z' />
              <path d='M736 376.6c-12.5-12.5-32.8-12.5-45.3 0L640 427.3l-50.7-50.7c-12.5-12.5-32.8-12.5-45.3 0s-12.5 32.8 0 45.3L594.7 472.6 544 523.3c-12.5 12.5-12.5 32.8 0 45.3 12.5 12.5 32.8 12.5 45.3 0L640 517.9l50.7 50.7c12.5 12.5 32.8 12.5 45.3 0s12.5-32.8 0-45.3L685.3 472.6l50.7-50.7c12.5-12.5 12.5-32.8 0-45.3z' />
            </svg>
            <svg v-else class='musicVolIcon' viewBox='0 0 1024 1024'>
              <path d='M524.8 161.6L283.2 384H128c-17.7 0-32 14.3-32 32v192c0 17.7 14.3 32 32 32h155.2l241.6 222.4c20.5 18.9 53.7 4.4 53.7-23.5V185.1c0-27.9-33.2-42.4-53.7-23.5z' />
              <path v-if='volume > 0.33' d='M704 320c-12.6-12.6-32.8-12.6-45.3 0-12.6 12.6-12.6 32.8 0 45.3 39.4 39.4 61.1 91.7 61.1 147.3s-21.7 107.9-61.1 147.3c-12.6 12.6-12.6 32.8 0 45.3 6.2 6.2 14.4 9.4 22.6 9.4 8.2 0 16.4-3.1 22.6-9.4 51.5-51.5 79.8-119.9 79.8-192.5s-28.4-141-79.7-192.7z' />
              <path v-if='volume > 0.66' d='M790.8 233.2c-12.6-12.6-32.8-12.6-45.3 0-12.6 12.6-12.6 32.8 0 45.3 62.6 62.6 97 145.7 97 234.1s-34.5 171.5-97 234.1c-12.6 12.6-12.6 32.8 0 45.3 6.2 6.2 14.4 9.4 22.6 9.4 8.2 0 16.4-3.1 22.6-9.4 74.7-74.7 115.7-173.9 115.7-279.4s-41-204.8-115.6-279.4z' />
            </svg>
          </a-button>
          <div class='musicVolBar' @mousedown='onVolumeBarMouseDown'>
            <div class='musicVolFill' :style="{ width: (muted ? 0 : volume * 100) + '%' }"></div>
          </div>
        </div>

        <a-popover position='top' trigger='click' :popup-visible='sleepPopVisible'
                   @popup-visible-change='(v:boolean)=>sleepPopVisible=v'>
          <button class='musicIconBtn musicSleepBtn' :class="{ active: sleepActive }" title='睡眠定时器'>
            <svg viewBox='0 0 1024 1024'>
              <path d='M524.8 928c-205.3 0-372.3-167-372.3-372.3 0-181.7 130.7-336.5 308.6-369.1 18.9-3.5 31.6 16.3 22.5 33.3-21.4 39.9-32.5 86.4-32.5 134.5 0 158.5 128.5 287 287 287 48.1 0 94.6-11.1 134.5-32.5 17-9.1 36.8 3.6 33.3 22.5C873.3 829.3 706.5 928 524.8 928z' />
            </svg>
            <span v-if='sleepActive' class='musicSleepLabel'>{{ sleepLabel }}</span>
          </button>
          <template #content>
            <div class='musicSleepMenu'>
              <div class='musicSleepMenuTitle'>睡眠定时器</div>
              <button class='musicSleepOpt' :class="{ active: sleepActive === false }" @click='setSleepTimer(0)'>
                关闭
              </button>
              <button class='musicSleepOpt' @click='setSleepTimer(8)'>8 分钟</button>
              <button class='musicSleepOpt' @click='setSleepTimer(15)'>15 分钟</button>
              <button class='musicSleepOpt' @click='setSleepTimer(30)'>30 分钟</button>
              <button class='musicSleepOpt' @click='setSleepTimer(60)'>60 分钟</button>
              <button class='musicSleepOpt' :class="{ active: sleepEndOfTrack }" @click="setSleepTimer('end-of-track')">
                播完当前曲后停止
              </button>
            </div>
          </template>
        </a-popover>
      </div>
    </div>

    <audio
      ref='audioRef'
      preload='auto'
      @loadedmetadata='onAudioLoaded'
      @durationchange='onAudioLoaded'
      @timeupdate='onAudioTimeUpdate'
      @ended='onAudioEnded'
      @error='onAudioError'
      @play='onAudioPlay'
      @pause='onAudioPause'
      style='display: none' />
  </a-layout>
</template>

<style scoped lang='less'>
.pageMusic {
  position: relative;
  background: linear-gradient(135deg, #0d0e16 0%, #1a1224 60%, #0c0c14 100%);
  color: #f0f0f5;
  user-select: none;
  overflow: hidden;
}

.musicBg {
  position: absolute;
  inset: -8% -8% -8% -8%;
  background-size: cover;
  background-position: center;
  background-repeat: no-repeat;
  filter: blur(60px) brightness(0.55) saturate(1.4);
  transform: scale(1.2);
  z-index: 0;
  will-change: opacity;
}

.musicBgOverlay {
  position: absolute;
  inset: 0;
  z-index: 1;
  background:
    linear-gradient(180deg, rgba(0,0,0,0.18) 0%, rgba(0,0,0,0.25) 50%, rgba(0,0,0,0.55) 100%),
    radial-gradient(circle at 30% 20%, rgba(120,115,245,0.12) 0%, transparent 60%),
    radial-gradient(circle at 80% 80%, rgba(236,119,171,0.10) 0%, transparent 55%);
  pointer-events: none;
}

.bgFade-enter-active,
.bgFade-leave-active {
  transition: opacity 0.5s ease;
}
.bgFade-enter-from,
.bgFade-leave-to { opacity: 0; }
.bgFade-enter-to,
.bgFade-leave-from { opacity: 1; }

.musicHeader {
  position: relative;
  z-index: 2;
}

/* ============== Tab bar ============== */

.musicTabs {
  position: relative;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 18px 0;
  gap: 12px;
}

.musicTabsLeft {
  display: flex;
  align-items: center;
  gap: 4px;
}

.musicTabsRight {
  display: flex;
  align-items: center;
  gap: 8px;
}

.musicTabBtn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.65);
  background: transparent;
  border: none;
  border-radius: 18px;
  cursor: pointer;
  transition: color 0.15s ease, background 0.15s ease;
  position: relative;
}

.musicTabBtn:hover {
  color: #ffffff;
  background: rgba(255, 255, 255, 0.06);
}

.musicTabBtn.active {
  color: #ffffff;
  background: rgba(255, 255, 255, 0.12);
  font-weight: 600;
}

.musicTabIcon {
  font-size: 15px;
  width: 16px;
  height: 16px;
  fill: currentColor;
}

.musicTabBadge {
  font-size: 11px;
  padding: 1px 7px;
  background: rgba(255, 255, 255, 0.14);
  border-radius: 10px;
  color: rgba(255, 255, 255, 0.85);
  line-height: 1.4;
  font-variant-numeric: tabular-nums;
}

.musicTabBtn.active .musicTabBadge {
  background: rgba(120, 115, 245, 0.45);
  color: #ffffff;
}

.musicSearch {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: rgba(255, 255, 255, 0.08);
  border-radius: 18px;
  width: 240px;
  transition: background 0.15s ease;
}

.musicSearch:focus-within {
  background: rgba(255, 255, 255, 0.14);
}

.musicSearchIcon {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.55);
}

.musicSearchInput {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: #ffffff;
  font-size: 13px;
  min-width: 0;
}

.musicSearchInput::placeholder { color: rgba(255, 255, 255, 0.4); }

.musicSearchClear {
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.55);
  cursor: pointer;
  padding: 0;
  display: inline-flex;
  align-items: center;
}

.musicSearchClear:hover { color: #ffffff; }

.musicListAction :deep(.arco-btn) {
  color: rgba(255, 255, 255, 0.7);
}

/* ============== Main area ============== */

.musicMainArea {
  position: relative;
  z-index: 2;
  height: calc(100vh - 42px - 44px - 92px);
  overflow: hidden;
}

/* --------- Now playing tab --------- */

.musicNow {
  height: 100%;
  display: flex;
  flex-direction: row;
  align-items: stretch;
  gap: 32px;
  padding: 24px 48px;
  overflow: hidden;
}

.musicNowLeft {
  flex: 0 0 auto;
  width: 360px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  gap: 22px;
}

.musicVinyl {
  position: relative;
  width: 280px;
  height: 280px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.musicVinylRing {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  background:
    radial-gradient(circle at center, rgba(0,0,0,0.85) 0%, rgba(0,0,0,0.85) 38%, rgba(255,255,255,0.05) 39%, rgba(0,0,0,0.85) 41%, rgba(0,0,0,0.85) 100%),
    repeating-radial-gradient(circle at center, rgba(255,255,255,0.03) 0px, rgba(255,255,255,0.03) 2px, rgba(0,0,0,0) 2px, rgba(0,0,0,0) 4px);
  box-shadow: 0 14px 40px rgba(0, 0, 0, 0.55), inset 0 0 30px rgba(0,0,0,0.6);
  animation: musicSpin 22s linear infinite;
  animation-play-state: paused;
}

.musicVinyl.playing .musicVinylRing,
.musicVinyl.playing .musicVinylDisc {
  animation-play-state: running;
}

.musicVinylDisc {
  position: relative;
  width: 70%;
  height: 70%;
  border-radius: 50%;
  overflow: hidden;
  background: rgba(255, 255, 255, 0.05);
  box-shadow: 0 0 0 6px rgba(0,0,0,0.25), 0 0 0 7px rgba(255,255,255,0.05);
  animation: musicSpin 22s linear infinite;
  animation-play-state: paused;
}

.musicVinylDisc img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.musicVinylCenter {
  position: absolute;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: linear-gradient(135deg, #b9b9c8, #6a6a7a);
  box-shadow: 0 0 0 2px rgba(0,0,0,0.55);
  pointer-events: none;
}

@keyframes musicSpin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.musicCoverFallback {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, rgba(120, 115, 245, 0.6), rgba(236, 119, 171, 0.55));
}

.musicCoverFallback .iconfont {
  font-size: 76px;
  color: rgba(255, 255, 255, 0.95);
}

.musicNowMeta {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
}

.musicNowTitle {
  font-size: 22px;
  font-weight: 700;
  color: #ffffff;
  width: 100%;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  text-shadow: 0 2px 12px rgba(0,0,0,0.5);
}

.musicNowArtist {
  font-size: 14px;
  color: rgba(255, 255, 255, 0.78);
  display: flex;
  gap: 6px;
  align-items: center;
  max-width: 100%;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.musicArtistDot {
  color: rgba(255, 255, 255, 0.4);
}

.musicNowSub {
  margin-top: 4px;
  display: flex;
  gap: 6px;
  align-items: center;
  flex-wrap: wrap;
  justify-content: center;
}

.musicNowChip {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.1);
  color: rgba(255, 255, 255, 0.7);
  font-variant-numeric: tabular-nums;
}

.musicError {
  font-size: 12px;
  color: #ff7875;
}

.musicSubMuted {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.55);
}

/* --------- Lyrics in now-playing --------- */

.musicNowLyrics {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  padding: 0 24px;
}

.musicLyricsList {
  flex: 1;
  overflow-y: auto;
  padding: 12px 0;
  scroll-behavior: smooth;
  mask-image: linear-gradient(180deg, transparent 0%, rgba(0,0,0,1) 12%, rgba(0,0,0,1) 88%, transparent 100%);
  -webkit-mask-image: linear-gradient(180deg, transparent 0%, rgba(0,0,0,1) 12%, rgba(0,0,0,1) 88%, transparent 100%);
}

.musicLyricsList::-webkit-scrollbar { width: 6px; }
.musicLyricsList::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.08);
  border-radius: 3px;
}

.musicLyricsSpacer {
  height: 38%;
  min-height: 100px;
}

.musicLyricsLine {
  text-align: center;
  padding: 9px 16px;
  font-size: 17px;
  line-height: 1.6;
  color: rgba(255, 255, 255, 0.45);
  cursor: pointer;
  transition: color 0.2s ease, transform 0.25s ease, font-size 0.2s ease, font-weight 0.2s ease;
  word-break: break-word;
}

.musicLyricsLine:hover {
  color: rgba(255, 255, 255, 0.85);
}

.musicLyricsLine.past {
  color: rgba(255, 255, 255, 0.28);
}

.musicLyricsLine.active {
  color: #ffffff;
  font-size: 22px;
  font-weight: 700;
  transform: scale(1.04);
  text-shadow: 0 0 22px rgba(120, 115, 245, 0.65), 0 2px 8px rgba(0,0,0,0.4);
}

.musicLyricsEmpty {
  padding: 60px 16px;
  text-align: center;
  color: rgba(255, 255, 255, 0.45);
  font-size: 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  align-items: center;
  justify-content: center;
  height: 100%;
}

.musicLyricsEmptyHint {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.35);
  max-width: 360px;
  line-height: 1.6;
}

/* --------- List view (queue / fav / recent) --------- */

.musicListView {
  height: 100%;
  overflow-y: auto;
  padding: 8px 18px 24px;
}

.musicListView::-webkit-scrollbar { width: 8px; }
.musicListView::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.1);
  border-radius: 4px;
}

.musicListGrid {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.musicListRow {
  display: grid;
  grid-template-columns: 32px 48px 1fr auto;
  align-items: center;
  gap: 14px;
  padding: 8px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.12s ease;
  color: rgba(255, 255, 255, 0.85);
}

.musicListRow:hover {
  background: rgba(255, 255, 255, 0.06);
}

.musicListRow.current {
  background: rgba(120, 115, 245, 0.16);
  color: #ffffff;
}

.musicListIdx {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.42);
  font-variant-numeric: tabular-nums;
  text-align: center;
}

.musicListRow.current .musicListIdx { color: rgb(var(--primary-6)); }

.musicListThumb {
  width: 48px;
  height: 48px;
  border-radius: 6px;
  overflow: hidden;
  background: rgba(255, 255, 255, 0.05);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.musicListThumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.musicListThumb .iconfont {
  font-size: 22px;
  color: rgba(255, 255, 255, 0.5);
}

.musicListMeta {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.musicListName {
  font-size: 14px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.musicListSub {
  display: flex;
  gap: 6px;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
}

.musicListExt {
  background: rgba(255, 255, 255, 0.08);
  padding: 1px 6px;
  border-radius: 4px;
  font-size: 11px;
}

.musicListActions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.musicIconBtn {
  width: 32px;
  height: 32px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 50%;
  cursor: pointer;
  color: rgba(255, 255, 255, 0.65);
  transition: background 0.12s ease, color 0.12s ease;
  padding: 0;
}

.musicIconBtn:hover { background: rgba(255, 255, 255, 0.1); color: #ffffff; }

.musicIconBtn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
  background: transparent !important;
  color: rgba(255, 255, 255, 0.3) !important;
}

.musicIconBtn .iconfont { font-size: 14px; }

.musicIconBtn svg {
  width: 16px;
  height: 16px;
  fill: currentColor;
}

.musicIconBtn.active {
  color: #ec77ab;
}

.musicListEmpty {
  padding: 80px 16px;
  text-align: center;
  color: rgba(255, 255, 255, 0.45);
  font-size: 14px;
}

/* ============== Bottom persistent bar ============== */

.musicBottomBar {
  position: relative;
  z-index: 3;
  height: 92px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(360px, 1.5fr) minmax(0, 1fr);
  align-items: center;
  gap: 16px;
  padding: 10px 20px;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(20px) saturate(1.2);
  -webkit-backdrop-filter: blur(20px) saturate(1.2);
  border-top: 1px solid rgba(255, 255, 255, 0.06);
}

.musicBottomLeft {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.musicMiniCover {
  width: 56px;
  height: 56px;
  border-radius: 8px;
  overflow: hidden;
  background: rgba(255, 255, 255, 0.05);
  flex-shrink: 0;
  position: relative;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.45);
}

.musicMiniCover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.musicMiniCoverFallback {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, rgba(120, 115, 245, 0.6), rgba(236, 119, 171, 0.55));
}

.musicMiniCoverFallback .iconfont {
  font-size: 22px;
  color: rgba(255, 255, 255, 0.95);
}

.musicMiniMeta {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.musicMiniTitle {
  font-size: 14px;
  font-weight: 600;
  color: #ffffff;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.musicMiniArtist {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.55);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.musicFavBtn {
  width: 36px;
  height: 36px;
  flex-shrink: 0;
}

.musicFavBtn svg {
  width: 18px;
  height: 18px;
}

.musicBottomCenter {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.musicBottomCtrls {
  display: flex;
  align-items: center;
  gap: 6px;
}

.musicBottomCtrls :deep(.arco-btn) {
  color: rgba(255, 255, 255, 0.85);
  background: transparent;
  border: none;
  width: 36px;
  height: 36px;
  min-width: 36px;
  position: relative;
  padding: 0;
}

.musicBottomCtrls :deep(.arco-btn):hover {
  background: rgba(255, 255, 255, 0.1);
  color: #ffffff;
}

.musicBottomCtrls :deep(.arco-btn .iconfont) {
  font-size: 20px;
}

.musicBottomCtrls :deep(.musicPlayBtn) {
  width: 44px;
  height: 44px;
  min-width: 44px;
  background: linear-gradient(135deg, #7873f5, #ec77ab);
  color: #ffffff;
  margin: 0 4px;
  box-shadow: 0 4px 14px rgba(120, 115, 245, 0.5);
}

.musicBottomCtrls :deep(.musicPlayBtn):hover {
  background: linear-gradient(135deg, #8c87ff, #ff85b8);
  color: #ffffff;
}

.musicBottomCtrls :deep(.musicPlayBtn .iconfont) {
  font-size: 22px;
}

.musicBottomCtrls :deep(.musicPlaylistOn) {
  color: rgb(var(--primary-6));
  background: rgba(120, 115, 245, 0.18);
}

.musicPlayIcon {
  width: 20px;
  height: 20px;
  fill: #ffffff;
  display: block;
}

.musicVolIcon {
  width: 18px;
  height: 18px;
  fill: rgba(255, 255, 255, 0.85);
  display: block;
}

.musicSidePlayIcon {
  width: 14px;
  height: 14px;
  fill: rgb(var(--primary-6));
  display: block;
  margin: 0 auto;
}

.musicModeBadge {
  position: absolute;
  bottom: 4px;
  right: 4px;
  font-size: 10px;
  font-weight: 700;
  background: rgb(var(--primary-6));
  color: #ffffff;
  border-radius: 6px;
  padding: 0 3px;
  line-height: 12px;
}

.musicBottomProgress {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  max-width: 540px;
}

.musicTime {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.6);
  font-variant-numeric: tabular-nums;
  min-width: 38px;
  text-align: center;
}

.musicBar {
  flex: 1;
  position: relative;
  height: 4px;
  background: rgba(255, 255, 255, 0.14);
  border-radius: 2px;
  cursor: pointer;
}

.musicBar:hover { height: 6px; margin: -1px 0; }
.musicBar:hover .musicBarThumb { opacity: 1; }

.musicBarFill {
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  background: linear-gradient(90deg, #7873f5, #ec77ab);
  border-radius: 2px;
  pointer-events: none;
}

.musicBarThumb {
  position: absolute;
  top: 50%;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #ffffff;
  transform: translate(-50%, -50%);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
  opacity: 0;
  transition: opacity 0.15s ease;
  pointer-events: none;
}

.musicBottomRight {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}

.musicVolume {
  display: flex;
  align-items: center;
  gap: 4px;
}

.musicVolume :deep(.arco-btn) {
  color: rgba(255, 255, 255, 0.85);
  background: transparent;
  border: none;
  width: 32px;
  height: 32px;
  min-width: 32px;
  padding: 0;
}

.musicVolume :deep(.arco-btn):hover {
  background: rgba(255, 255, 255, 0.1);
  color: #ffffff;
}

.musicVolBar {
  width: 90px;
  height: 4px;
  background: rgba(255, 255, 255, 0.12);
  border-radius: 2px;
  cursor: pointer;
  position: relative;
}

.musicVolFill {
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  background: rgba(255, 255, 255, 0.85);
  border-radius: 2px;
  pointer-events: none;
}

.musicSleepBtn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  width: auto;
  height: 32px;
  padding: 0 10px;
  border-radius: 16px;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

.musicSleepBtn.active {
  background: rgba(120, 115, 245, 0.22);
  color: rgb(var(--primary-6));
}

.musicSleepBtn svg {
  width: 16px;
  height: 16px;
}

.musicSleepLabel {
  font-size: 12px;
}

.musicSleepMenu {
  display: flex;
  flex-direction: column;
  min-width: 160px;
  padding: 4px 0;
}

.musicSleepMenuTitle {
  font-size: 12px;
  color: rgba(0, 0, 0, 0.5);
  padding: 4px 12px 6px;
}

.musicSleepOpt {
  text-align: left;
  background: transparent;
  border: none;
  padding: 8px 12px;
  font-size: 13px;
  color: var(--color-text-1, #1d2129);
  cursor: pointer;
  border-radius: 4px;
}

.musicSleepOpt:hover {
  background: rgba(120, 115, 245, 0.1);
}

.musicSleepOpt.active {
  color: rgb(var(--primary-6));
  background: rgba(120, 115, 245, 0.12);
  font-weight: 600;
}

/* ============== Transitions ============== */

.musicFade-enter-active,
.musicFade-leave-active {
  transition: opacity 0.22s ease;
}

.musicFade-enter-from,
.musicFade-leave-to { opacity: 0; }

.musicFade-enter-to,
.musicFade-leave-from { opacity: 1; }

/* ============== Responsive ============== */

@media (max-width: 960px) {
  .musicNow {
    flex-direction: column;
    padding: 16px 16px 0;
    gap: 10px;
  }
  .musicNowLeft { width: 100%; gap: 12px; }
  .musicVinyl { width: 200px; height: 200px; }
  .musicNowLyrics { padding: 0 8px; }
  .musicBottomBar {
    grid-template-columns: minmax(0, 1.4fr) minmax(0, 1.6fr);
    height: auto;
    padding: 8px 12px;
  }
  .musicBottomRight { display: none; }
  .musicSearch { width: 180px; }
}

@media (max-width: 760px) {
  .musicTabBtn span:not(.musicTabBadge) { display: none; }
  .musicTabBtn { padding: 6px 10px; }
  .musicNowLeft { width: 100%; }
  .musicVinyl { width: 180px; height: 180px; }
  .musicListRow { grid-template-columns: 28px 40px 1fr auto; }
}
</style>
