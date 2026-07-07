<script setup lang='ts'>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { SkipBack, SkipForward, Pause, Play, FileText, Music, Search, X, List, Maximize2, Minus, Volume2, VolumeX, Heart, Clock, Timer, Shuffle, Repeat, Repeat1 } from 'lucide-vue-next'
import { KeyboardState, useAppStore, useKeyboardStore } from '../store'
import useMusicPlayerStore, { type MusicPlayerState } from '../store/musicplayerstore'
import message from '../utils/message'
import { TestAlt, TestKey, TestShift } from '../utils/keyboardhelper'
import { getRawUrl } from '../utils/proxyhelper'
import { IPageMusicTrack } from '../store/appstore'
import { fetchMusicMetadata, findActiveLineIndex, LyricLine, MusicMetadata } from '../utils/musicMetadata'
import DebugLog from '../utils/debuglog'
import { bindAudio, ensureInit as initAudioEngine } from '../module/audioplayer/index'
import SoundEffectBtn from '../components/SoundEffectBtn.vue'
import { shouldHandleMusicPlayerDblClick } from '../utils/pageMusicEvents'

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
  if (TestKey('arrowup', state.KeyDownEvent, () => setVol(Math.min(1, vol.value + 0.05)))) return
  if (TestKey('arrowdown', state.KeyDownEvent, () => setVol(Math.max(0, vol.value - 0.05)))) return
})

const onKeyDown = (e: KeyboardEvent) => {
  const el = (e.target || e.srcElement) as any
  if (document.querySelector('.arco-modal-container')) return
  if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return
  if (el?.nodeName === 'INPUT' || el?.nodeName === 'TEXTAREA') return
  keyboardStore.KeyDown(e)
}

type PlayMode = 'list' | 'loop-list' | 'loop-one' | 'shuffle'
type PanelTab = 'queue' | 'fav' | 'recent'

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
const panelTab = ref<PanelTab>('queue')
const errMsg = ref('')
const seekPreview = ref<number | null>(null)
const shuffleHist = ref<number[]>([])
const favs = ref<IPageMusicTrack[]>([])
const recents = ref<IPageMusicTrack[]>([])
const listFilter = ref('')

const sleepEndAt = ref<number | null>(null)
const sleepEndTrack = ref(false)
const sleepRemain = ref(0)
const sleepPop = ref(false)
let sleepTicker: number | null = null

const meta = ref<MusicMetadata | null>(null)
const metaLoad = ref(false)
const lyricRef = ref<HTMLElement | null>(null)
let metaTok = 0
let lastScrollIdx = -1
let rafId: number | null = null

const curTrack = computed(() => playlist.value[curIdx.value])
const title = computed(() => stripExt(curTrack.value?.file_name || ''))
const ext = computed(() => {
  const t = curTrack.value; if (!t) return ''
  if (t.ext) return t.ext.replace(/^\./, '').toUpperCase()
  const d = (t.file_name || '').lastIndexOf('.'); return d < 0 ? '' : t.file_name!.slice(d + 1).toUpperCase()
})
const coverUrl = computed(() => meta.value?.cover || curTrack.value?.thumbnail || '')
const artist = computed(() => meta.value?.artist || '')
const album = computed(() => meta.value?.album || '')
const lyricLines = computed<LyricLine[]>(() => meta.value?.lines || [])
const hasLyrics = computed(() => lyricLines.value.length > 0)
const lyricDebugText = computed(() => meta.value?.debug?.reason || '')
const lyricDebugTitle = computed(() => formatLyricDebugTitle(meta.value))
const activeLine = computed(() => findActiveLineIndex(lyricLines.value, curTime.value))
const progPct = computed(() => dur.value ? Math.max(0, Math.min(100, ((seekPreview.value ?? curTime.value) / dur.value) * 100)) : 0)
const dispTime = computed(() => fmtTime(seekPreview.value ?? curTime.value))
const isFav = computed(() => { const t = curTrack.value; return t ? favSet.value.has(favKey(t)) : false })
const favSet = computed(() => new Set(favs.value.map(favKey)))
const sleepOn = computed(() => sleepEndAt.value !== null || sleepEndTrack.value)
const sleepLbl = computed(() => sleepEndTrack.value ? '播完即停' : sleepEndAt.value ? fmtTime(sleepRemain.value) : '')
const modeLbl = computed(() => ({ list: '顺序', 'loop-list': '循环', 'loop-one': '单曲', shuffle: '随机' }[mode.value] || ''))
const modeOn = computed(() => mode.value !== 'list')
const fQ = computed(() => playlist.value.filter(t => matchT(t, listFilter.value)))
const fF = computed(() => favs.value.filter(t => matchT(t, listFilter.value)))
const fR = computed(() => recents.value.filter(t => matchT(t, listFilter.value)))

function favKey(t: any) { return t ? `${t.user_id || ''}|${t.drive_id || ''}|${t.file_id || ''}` : '' }
function stripExt(n: string) { const i = n.lastIndexOf('.'); return i > 0 ? n.slice(0, i) : n }
function fmtTime(s: number) { s = Math.max(0, Math.floor(s)); const m = Math.floor(s / 60), sec = s % 60; return `${m}:${sec.toString().padStart(2, '0')}` }
function matchT(t: IPageMusicTrack, q: string) { return !q || (t.file_name || '').toLowerCase().includes(q.toLowerCase()) }
function isCur(t: IPageMusicTrack) { const c = curTrack.value; return c ? favKey(c) === favKey(t) : false }
function loadJson<T>(k: string, fb: T): T { try { const s = localStorage.getItem(k); return s ? JSON.parse(s) ?? fb : fb } catch { return fb } }
function saveJson(k: string, v: unknown) { try { localStorage.setItem(k, JSON.stringify(v)) } catch {} }
function saveFavs() { saveJson('pm.favs', favs.value) }
function saveRecs() { saveJson('pm.recs', recents.value) }
function formatLyricDebugTitle(data: MusicMetadata | null) {
  const debug = data?.debug
  if (!debug) return ''
  const lines = [
    `歌词接口: musicsdk (网易云 wy -> 酷狗 kg)`,
    `查询: ${debug.query.artist ? `${debug.query.artist} - ` : ''}${debug.query.title}`,
    `文件: ${debug.query.filename}`,
    `结果: ${debug.reason}`,
    `LRC字符: ${debug.lrcLength}`,
    `解析行数: ${debug.parsedLineCount}`
  ]
  if (debug.lyric?.matched) {
    lines.push(`匹配歌曲: ${debug.lyric.matched.source} ${debug.lyric.matched.name} / ${debug.lyric.matched.singer}`)
  }
  for (const step of debug.lyric?.steps || []) {
    lines.push(`${step.source}: ${step.status} ${step.message}${step.lyricLength !== undefined ? ` lyric=${step.lyricLength}` : ''}`)
  }
  return lines.join('\n')
}

function toggleFav(t: IPageMusicTrack | undefined | null) {
  if (!t) return; const k = favKey(t); const i = favs.value.findIndex(f => favKey(f) === k)
  if (i >= 0) favs.value.splice(i, 1); else favs.value.unshift({ ...t }); saveFavs()
}
function removeFav(t: IPageMusicTrack) { favs.value = favs.value.filter(f => favKey(f) !== favKey(t)); saveFavs() }
function pushRec(t: IPageMusicTrack | undefined | null) {
  if (!t) return; const k = favKey(t); const i = recents.value.findIndex(r => favKey(r) === k)
  if (i >= 0) recents.value.splice(i, 1); recents.value.unshift({ ...t })
  if (recents.value.length > 50) recents.value.length = 50; saveRecs()
}

async function resolveUrl(idx: number) {
  const t = playlist.value[idx]; if (!t) return ''
  const d = await getRawUrl(t.user_id, t.drive_id, t.file_id, t.encType || '', t.password || '', false, 'audio')
  if (typeof d === 'string') throw new Error(d || '获取地址失败')
  return d.url || ''
}

async function loadIdx(idx: number, auto = true) {
  if (!playlist.value.length) return
  idx = Math.max(0, Math.min(idx, playlist.value.length - 1))
  curIdx.value = idx; errMsg.value = ''; dur.value = 0; curTime.value = 0
  const a = audioRef.value; if (!a) return
  try {
    loading.value = true
    const url = await resolveUrl(idx); if (!url) throw new Error('未获取到播放地址')
    a.src = url; a.load()
    if (auto) { try { await a.play(); playing.value = true } catch { playing.value = false } }
  } catch (e: any) {
    errMsg.value = e?.message || String(e); message.error('加载失败: ' + errMsg.value); playing.value = false
  } finally { loading.value = false }
  loadMeta(); pushRec(curTrack.value)
}

async function loadMeta() {
  const t = curTrack.value; metaTok++; const tok = metaTok
  if (!t) { meta.value = null; return }
  metaLoad.value = true
  try {
    const d = await fetchMusicMetadata({ filename: t.file_name, durationSec: dur.value || undefined })
    if (tok !== metaTok) return
    meta.value = d; lastScrollIdx = -1
    if (showLyrics.value) scheduleScroll(true)
  } catch { if (tok === metaTok) meta.value = null }
  finally { if (tok === metaTok) metaLoad.value = false }
}

function scheduleScroll(force = false) {
  if (rafId !== null) cancelAnimationFrame(rafId)
  rafId = requestAnimationFrame(() => {
    rafId = null
    doScroll(force)
  })
}

function doScroll(force = false) {
  const el = lyricRef.value; if (!el) return
  const idx = activeLine.value
  if (!force && idx === lastScrollIdx) return
  lastScrollIdx = idx
  const tgt = el.querySelector<HTMLElement>(`[data-li="${idx}"]`)
  if (!tgt) { el.scrollTop = 0; return }
  el.scrollTo({ top: Math.max(0, tgt.offsetTop - el.clientHeight / 2 + tgt.clientHeight / 2), behavior: force ? 'auto' : 'smooth' })
}

function togglePlay() {
  const a = audioRef.value; if (!a) return
  if (!a.src) return loadIdx(curIdx.value, true)
  if (a.paused) a.play().then(() => playing.value = true).catch(() => playing.value = false)
  else { a.pause(); playing.value = false }
}

function playPrev() {
  if (!playlist.value.length) return
  if (mode.value === 'shuffle') {
    if (shuffleHist.value.length > 1) { shuffleHist.value.pop(); return loadIdx(shuffleHist.value[shuffleHist.value.length - 1], true) }
    return loadIdx(pickShuffle(), true)
  }
  loadIdx(curIdx.value <= 0 ? playlist.value.length - 1 : curIdx.value - 1, true)
}

function playNext(auto = false) {
  if (!playlist.value.length) return
  if (mode.value === 'loop-one' && auto) return loadIdx(curIdx.value, true)
  if (mode.value === 'shuffle') return loadIdx(pickShuffle(), true)
  let n = curIdx.value + 1
  if (n >= playlist.value.length) { if (mode.value === 'list' && auto) { playing.value = false; return }; n = 0 }
  loadIdx(n, true)
}

function pickShuffle() {
  if (playlist.value.length <= 1) return 0
  let n = curIdx.value, tries = 0
  while (n === curIdx.value && tries++ < 32) n = Math.floor(Math.random() * playlist.value.length)
  shuffleHist.value.push(n); if (shuffleHist.value.length > 100) shuffleHist.value.shift()
  return n
}

function playFrom(t: IPageMusicTrack) {
  const k = favKey(t); let i = playlist.value.findIndex(p => favKey(p) === k)
  if (i < 0) { playlist.value.push({ ...t }); i = playlist.value.length - 1 }
  showPanel.value = false; loadIdx(i, true)
}

function seekRel(d: number) { const a = audioRef.value; if (a && dur.value) a.currentTime = Math.max(0, Math.min(dur.value, (a.currentTime || 0) + d)) }
function setVol(v: number) {
  vol.value = Math.max(0, Math.min(1, v))
  if (audioRef.value) { audioRef.value.volume = vol.value; if (vol.value > 0 && muted.value) { muted.value = false; audioRef.value.muted = false } }
  saveJson('pm.vol', vol.value)
}
function toggleMute() { if (!audioRef.value) return; muted.value = !muted.value; audioRef.value.muted = muted.value }
function cycleMode() { const o: PlayMode[] = ['loop-list', 'loop-one', 'shuffle', 'list']; mode.value = o[(o.indexOf(mode.value) + 1) % o.length]; shuffleHist.value = []; saveJson('pm.mode', mode.value) }
function toggleLyrics() {
  showLyrics.value = !showLyrics.value
  nextTick(() => {
    const msg = `[PageMusic] lyrics toggle show=${showLyrics.value} sidePanel=${!!props.sidePanel} hasLyrics=${hasLyrics.value} lines=${lyricLines.value.length} lyricRef=${!!lyricRef.value}`
    console.info(msg)
    if (showLyrics.value && hasLyrics.value && !lyricRef.value) DebugLog.mSaveWarning(msg + '；歌词数据已解析，但歌词 DOM 未挂载')
  })
}

function onProgDown(e: MouseEvent) {
  const bar = e.currentTarget as HTMLElement; if (!bar || !dur.value) return
  const rect = bar.getBoundingClientRect()
  const upd = (cx: number) => seekPreview.value = (Math.max(0, Math.min(rect.width, cx - rect.left)) / rect.width) * dur.value
  upd(e.clientX)
  const mv = (ev: MouseEvent) => upd(ev.clientX)
  const up = (ev: MouseEvent) => {
    upd(ev.clientX); window.removeEventListener('mousemove', mv); window.removeEventListener('mouseup', up)
    if (audioRef.value && seekPreview.value != null) audioRef.value.currentTime = seekPreview.value
    seekPreview.value = null
  }
  window.addEventListener('mousemove', mv); window.addEventListener('mouseup', up)
}

function onVolDown(e: MouseEvent) {
  const bar = e.currentTarget as HTMLElement; if (!bar) return
  const rect = bar.getBoundingClientRect()
  const upd = (cx: number) => setVol(Math.max(0, Math.min(rect.width, cx - rect.left)) / rect.width)
  upd(e.clientX)
  const mv = (ev: MouseEvent) => upd(ev.clientX)
  const up = () => { window.removeEventListener('mousemove', mv); window.removeEventListener('mouseup', up) }
  window.addEventListener('mousemove', mv); window.addEventListener('mouseup', up)
}

function clearSleep() {
  if (sleepTicker) { window.clearInterval(sleepTicker); sleepTicker = null }
  sleepEndAt.value = null; sleepRemain.value = 0; sleepEndTrack.value = false
}
function setSleep(opt: number | 'end' | 0) {
  clearSleep(); sleepPop.value = false
  if (opt === 0) return
  if (opt === 'end') { sleepEndTrack.value = true; return }
  sleepEndAt.value = Date.now() + opt * 60000; sleepRemain.value = opt * 60
  sleepTicker = window.setInterval(() => {
    if (!sleepEndAt.value) { if (sleepTicker) { window.clearInterval(sleepTicker); sleepTicker = null }; return }
    const r = Math.max(0, Math.ceil((sleepEndAt.value - Date.now()) / 1000))
    sleepRemain.value = r
    if (r <= 0) { try { audioRef.value?.pause() } catch {}; playing.value = false; clearSleep() }
  }, 1000)
}

function handleHide() { try { audioRef.value?.pause() } catch {}; window.WebToWindow?.({ cmd: 'close' }); if (!window.WebToWindow) window.close() }
function handleMin() { window.WebToWindow?.({ cmd: 'minsize' }) }
function handleMax() { window.WebToWindow?.({ cmd: 'maxsize' }) }
function handleRootDblClick(e: MouseEvent) { if (shouldHandleMusicPlayerDblClick(e)) handleMax() }

function onLoaded() { dur.value = audioRef.value?.duration || 0 }
function onTime() { curTime.value = audioRef.value?.currentTime || 0 }
function onEnded() { if (sleepEndTrack.value) { sleepEndTrack.value = false; playing.value = false; try { audioRef.value?.pause() } catch {}; return }; playNext(true) }
function onErr() { if (!audioRef.value?.src) return; errMsg.value = '播放出错'; playing.value = false }
function onPlay() { playing.value = true }
function onPause() { playing.value = false }

function emitState() {
  emit('state-change', { title: title.value || curTrack.value?.file_name || '', artist: artist.value, album: album.value, coverUrl: coverUrl.value, isPlaying: playing.value, isLoading: loading.value, currentTime: curTime.value, duration: dur.value, progressPercent: progPct.value, hasTrack: !!curTrack.value })
}

onMounted(() => {
  window.addEventListener('keydown', onKeyDown, true)
  vol.value = loadJson('pm.vol', 0.8)
  mode.value = (loadJson('pm.mode', 'loop-list') as PlayMode) || 'loop-list'
  showLyrics.value = loadJson('pm.lyrics', false)
  const tab = loadJson('pm.tab', 'queue'); if (['queue', 'fav', 'recent'].includes(String(tab))) panelTab.value = tab as PanelTab
  favs.value = (loadJson<IPageMusicTrack[]>('pm.favs', []) || []).filter(t => t?.file_id)
  recents.value = (loadJson<IPageMusicTrack[]>('pm.recs', []) || []).filter(t => t?.file_id)

  if (audioRef.value) { audioRef.value.volume = vol.value; audioRef.value.crossOrigin = 'anonymous'; bindAudio(audioRef.value); initAudioEngine() }

  if (props.sidePanel) return

  const d = appStore.pageMusic
  if (!d) { message.error('未提供音乐播放参数'); return }
  document.title = d.file_name || '音乐播放器'
  playlist.value = (d.playlist || []).slice()
  if (!playlist.value.length) playlist.value = [{ user_id: d.user_id, drive_id: d.drive_id, file_id: d.file_id, parent_file_id: d.parent_file_id, file_name: d.file_name, encType: d.encType, password: d.password }]
  let idx = playlist.value.findIndex(t => t.file_id === d.file_id); if (idx < 0) idx = 0
  curIdx.value = idx
  loadIdx(idx, true)
})

watch(curTrack, t => { if (t) document.title = t.file_name })
watch([curTrack, playing, loading, curTime, dur, coverUrl, artist, album, progPct], emitState, { immediate: true })
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
  playlist.value = (d.playlist || []).slice()
  if (!playlist.value.length) playlist.value = [{ user_id: d.user_id, drive_id: d.drive_id, file_id: d.file_id, parent_file_id: d.parent_file_id, file_name: d.file_name, encType: d.encType, password: d.password }]
  let idx = playlist.value.findIndex(t => t.file_id === d.file_id); if (idx < 0) idx = 0
  curIdx.value = idx
  loadIdx(idx, true)
})
watch(activeLine, () => { if (showLyrics.value) scheduleScroll(false) })
watch(showLyrics, v => {
  saveJson('pm.lyrics', v)
  if (v) scheduleScroll(true)
})
watch(panelTab, v => { saveJson('pm.tab', v); listFilter.value = '' })

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown, true)
  if (sleepTicker) { window.clearInterval(sleepTicker); sleepTicker = null }
  if (rafId !== null) { cancelAnimationFrame(rafId); rafId = null }
  try { audioRef.value?.pause() } catch {}
})

defineExpose({ togglePlay, playPrev, playNext, seekRel })
</script>

<template>
  <div :class="['amp', props.embedded ? 'emb' : '', props.sidePanel ? 'sp' : '']" @dblclick="handleRootDblClick">

    <!-- 模糊背景 -->
    <div class="amp-bg">
      <div class="amp-bg-blur" :style="coverUrl ? { backgroundImage: `url(${coverUrl})` } : {}"></div>
      <div class="amp-bg-overlay"></div>
    </div>

    <!-- 窗口标题栏 -->
    <div v-if="!props.embedded" class="amp-titlebar q-electron-drag">
      <div class="amp-wbtn amp-wbtn-close" @click.stop="handleHide"></div>
      <div class="amp-wbtn amp-wbtn-min" @click.stop="handleMin"></div>
      <div class="amp-wbtn amp-wbtn-max" @click.stop="handleMax"></div>
    </div>

    <!-- 主内容区 -->
    <div class="amp-main">

      <!-- 上半部：封面 / 歌词（互相切换，v-show 不销毁 DOM） -->
      <div class="amp-stage">
        <!-- 封面 -->
        <div v-show="!showLyrics" class="amp-stage-cover">
          <div :class="['amp-cover', playing ? 'playing' : '']">
            <img v-if="coverUrl" :src="coverUrl" class="amp-cover-img" @error="(e:any) => { e.currentTarget.style.display='none' }"/>
            <div v-else class="amp-cover-fb"><Music :size="56" :stroke-width="1" /></div>
          </div>
        </div>

        <!-- 歌词 -->
        <div v-show="showLyrics" class="amp-stage-lyric">
          <div v-if="!hasLyrics && !metaLoad" class="amp-lyric-empty">
            <FileText :size="32" :stroke-width="1" />
            <span>暂未找到歌词</span>
            <small v-if="lyricDebugText" :title="lyricDebugTitle">{{ lyricDebugText }}</small>
          </div>
          <div v-else ref="lyricRef" class="amp-lyric-scroll">
            <div class="amp-lyric-spacer"></div>
            <div
              v-for="(l, i) in lyricLines" :key="i" :data-li="i"
              :class="['amp-lyric-line', i === activeLine ? 'active' : '', i < activeLine ? 'past' : '']"
              @click="audioRef && (audioRef.currentTime = l.time)"
            >{{ l.text }}</div>
            <div class="amp-lyric-spacer"></div>
          </div>
        </div>
      </div>

      <!-- 下半部：信息 + 进度 + 控制（始终可见） -->
      <div class="amp-bottom">

        <!-- 歌曲信息 -->
        <div class="amp-meta">
          <div class="amp-meta-row">
            <div class="amp-meta-info">
              <div class="amp-title">{{ title || '未在播放' }}</div>
              <div class="amp-artist">
                {{ artist || (album ? album : '未知艺术家') }}<span v-if="artist && album" class="amp-album"> — {{ album }}</span>
              </div>
            </div>
            <button :class="['amp-icon-btn', 'amp-fav-btn', isFav ? 'active' : '']" :disabled="!curTrack" @click="toggleFav(curTrack)">
              <Heart :size="20" :stroke-width="2" :fill="isFav ? 'currentColor' : 'none'" />
            </button>
          </div>
          <div class="amp-status">
            <span v-if="ext" class="amp-tag">{{ ext }}</span>
            <span class="amp-tag">{{ curIdx + 1 }} / {{ playlist.length }}</span>
            <span v-if="errMsg" class="amp-err">{{ errMsg }}</span>
            <span v-else-if="loading" class="amp-hint">加载中…</span>
            <span v-else-if="metaLoad" class="amp-hint">获取歌词…</span>
            <span v-else-if="showLyrics && lyricDebugText" class="amp-hint" :title="lyricDebugTitle">{{ lyricDebugText }}</span>
          </div>
        </div>

        <!-- 进度条 -->
        <div class="amp-progress">
          <span class="amp-time">{{ dispTime }}</span>
          <div class="amp-bar" @mousedown="onProgDown">
            <div class="amp-bar-track">
              <div class="amp-bar-fill" :style="{ width: progPct + '%' }"></div>
            </div>
            <div class="amp-bar-thumb" :style="{ left: progPct + '%' }"></div>
          </div>
          <span class="amp-time">{{ fmtTime(dur) }}</span>
        </div>

        <!-- 播放控制 -->
        <div class="amp-controls">
          <button :class="['amp-ctrl-btn', modeOn ? 'active' : '']" :title="modeLbl" @click="cycleMode">
            <Shuffle v-if="mode === 'shuffle'" :size="17" :stroke-width="1.8" />
            <Repeat1 v-else-if="mode === 'loop-one'" :size="17" :stroke-width="1.8" />
            <Repeat v-else-if="mode === 'loop-list'" :size="17" :stroke-width="1.8" />
            <List v-else :size="17" :stroke-width="1.8" />
          </button>
          <button class="amp-ctrl-btn" title="上一首" @click="playPrev">
            <SkipBack :size="22" :stroke-width="1.8" />
          </button>
          <button class="amp-play-btn" @click="togglePlay">
            <Pause v-if="playing" :size="28" fill="currentColor" :stroke-width="0" />
            <Play v-else :size="28" fill="currentColor" :stroke-width="0" style="margin-left:3px" />
          </button>
          <button class="amp-ctrl-btn" title="下一首" @click="playNext(false)">
            <SkipForward :size="22" :stroke-width="1.8" />
          </button>
          <div class="amp-vol-wrap">
            <button class="amp-ctrl-btn" @click="toggleMute">
              <VolumeX v-if="muted || vol === 0" :size="17" :stroke-width="1.8" />
              <Volume2 v-else :size="17" :stroke-width="1.8" />
            </button>
            <div class="amp-bar amp-vol-bar" @mousedown="onVolDown">
              <div class="amp-bar-track">
                <div class="amp-bar-fill" :style="{ width: (muted ? 0 : vol * 100) + '%' }"></div>
              </div>
              <div class="amp-bar-thumb" :style="{ left: (muted ? 0 : vol * 100) + '%' }"></div>
            </div>
          </div>
        </div>

        <!-- 工具栏 -->
        <div class="amp-toolbar">
          <button :class="['amp-icon-btn', showPanel ? 'active' : '']" title="播放列表" @click="showPanel = !showPanel">
            <List :size="18" :stroke-width="1.8" />
          </button>
          <button :class="['amp-icon-btn', showLyrics ? 'active' : '']" title="歌词" @click="toggleLyrics">
            <FileText :size="18" :stroke-width="1.8" />
          </button>
          <SoundEffectBtn />
          <a-popover position="top" trigger="click" :popup-visible="sleepPop" @popup-visible-change="(v:boolean) => sleepPop = v">
            <button :class="['amp-icon-btn', sleepOn ? 'active' : '']" title="定时关闭">
              <Timer :size="18" :stroke-width="1.8" />
              <span v-if="sleepOn" class="amp-sleep-badge">{{ sleepLbl }}</span>
            </button>
            <template #content>
              <div class="amp-sleep-menu">
                <div class="amp-sleep-title">定时关闭</div>
                <button :class="['amp-sleep-item', !sleepOn ? 'on' : '']" @click="setSleep(0)">关闭</button>
                <button class="amp-sleep-item" @click="setSleep(8)">8 分钟</button>
                <button class="amp-sleep-item" @click="setSleep(15)">15 分钟</button>
                <button class="amp-sleep-item" @click="setSleep(30)">30 分钟</button>
                <button class="amp-sleep-item" @click="setSleep(60)">60 分钟</button>
                <button :class="['amp-sleep-item', sleepEndTrack ? 'on' : '']" @click="setSleep('end')">播完即停</button>
              </div>
            </template>
          </a-popover>
        </div>
      </div>
    </div>

    <!-- 播放列表抽屉 -->
    <div v-show="showPanel" class="amp-drawer">
      <div class="amp-drawer-head">
        <div class="amp-drawer-tabs">
          <button :class="['amp-dtab', panelTab === 'queue' ? 'on' : '']" @click="panelTab = 'queue'">待播 ({{ playlist.length }})</button>
          <button :class="['amp-dtab', panelTab === 'fav' ? 'on' : '']" @click="panelTab = 'fav'">收藏 ({{ favs.length }})</button>
          <button :class="['amp-dtab', panelTab === 'recent' ? 'on' : '']" @click="panelTab = 'recent'">最近 ({{ recents.length }})</button>
        </div>
        <div class="amp-drawer-search">
          <Search :size="13" :stroke-width="1.5" />
          <input v-model="listFilter" placeholder="搜索…" class="amp-drawer-input" />
          <button v-if="listFilter" class="amp-drawer-clear" @click="listFilter = ''"><X :size="12" /></button>
        </div>
        <button class="amp-drawer-close" title="关闭" @click="showPanel = false"><X :size="16" :stroke-width="2" /></button>
      </div>
      <div class="amp-drawer-list">
        <template v-if="panelTab === 'queue'">
          <div v-for="(t, i) in fQ" :key="favKey(t)" :class="['amp-drow', isCur(t) ? 'cur' : '']" @click="playFrom(t)">
            <span class="amp-dnum">{{ i + 1 }}</span>
            <img v-if="t.thumbnail" :src="t.thumbnail" class="amp-dthumb" @error="(e:any) => { e.currentTarget.style.display = 'none' }" />
            <div v-else class="amp-dthumb amp-dthumb-fb"><Music :size="14" :stroke-width="1.5" /></div>
            <div class="amp-dmeta">
              <div class="amp-dname">{{ stripExt(t.file_name) }}</div>
              <span v-if="t.ext" class="amp-dext">{{ String(t.ext).replace(/^\./, '').toUpperCase() }}</span>
            </div>
            <button :class="['amp-dfav', favSet.has(favKey(t)) ? 'liked' : '']" @click.stop="toggleFav(t)">
              <Heart :size="14" :stroke-width="1.5" :fill="favSet.has(favKey(t)) ? 'currentColor' : 'none'" />
            </button>
          </div>
          <div v-if="!fQ.length" class="amp-dempty">{{ playlist.length ? '无匹配' : '列表为空' }}</div>
        </template>
        <template v-if="panelTab === 'fav'">
          <div v-for="t in fF" :key="favKey(t)" :class="['amp-drow', isCur(t) ? 'cur' : '']" @click="playFrom(t)">
            <span class="amp-dnum"><Heart :size="13" fill="currentColor" :stroke-width="0" /></span>
            <img v-if="t.thumbnail" :src="t.thumbnail" class="amp-dthumb" @error="(e:any) => { e.currentTarget.style.display = 'none' }" />
            <div v-else class="amp-dthumb amp-dthumb-fb"><Music :size="14" :stroke-width="1.5" /></div>
            <div class="amp-dmeta"><div class="amp-dname">{{ stripExt(t.file_name) }}</div></div>
            <button class="amp-dfav liked" @click.stop="removeFav(t)"><X :size="13" /></button>
          </div>
          <div v-if="!fF.length" class="amp-dempty">{{ favs.length ? '无匹配' : '暂无收藏' }}</div>
        </template>
        <template v-if="panelTab === 'recent'">
          <div v-for="t in fR" :key="favKey(t)" :class="['amp-drow', isCur(t) ? 'cur' : '']" @click="playFrom(t)">
            <span class="amp-dnum"><Clock :size="13" :stroke-width="1.5" /></span>
            <img v-if="t.thumbnail" :src="t.thumbnail" class="amp-dthumb" @error="(e:any) => { e.currentTarget.style.display = 'none' }" />
            <div v-else class="amp-dthumb amp-dthumb-fb"><Music :size="14" :stroke-width="1.5" /></div>
            <div class="amp-dmeta"><div class="amp-dname">{{ stripExt(t.file_name) }}</div></div>
          </div>
          <div v-if="!fR.length" class="amp-dempty">{{ recents.length ? '无匹配' : '暂无记录' }}</div>
          <button v-if="recents.length" class="amp-dclear" @click="recents = []; saveRecs()">清空历史</button>
        </template>
      </div>
    </div>

    <audio ref="audioRef" preload="auto"
      @loadedmetadata="onLoaded" @durationchange="onLoaded" @timeupdate="onTime"
      @ended="onEnded" @error="onErr" @play="onPlay" @pause="onPause"
      style="display:none" />
  </div>
</template>

<style scoped lang="less">
// ─── Root ───────────────────────────────────────────────────────────────────
.amp {
  position: relative;
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Display", "Helvetica Neue", sans-serif;
  color: #fff;
  user-select: none;
  overflow: hidden;
  -webkit-font-smoothing: antialiased;
  background: #111;

  &.emb {
    height: 100%;
  }

  &.sp {
    .amp-titlebar { display: none; }
    .amp-cover { width: min(180px, 40vw, 28vh); height: min(180px, 40vw, 28vh); }
    .amp-progress { display: none; }
    .amp-vol-wrap { display: none; }
    .amp-lyric-line { font-size: 14px; padding: 7px 12px; }
  }
}

// ─── Background ─────────────────────────────────────────────────────────────
.amp-bg {
  position: absolute;
  inset: 0;
  z-index: 0;
}
.amp-bg-blur {
  position: absolute;
  inset: -10%;
  background-size: cover;
  background-position: center;
  filter: blur(40px) brightness(0.28) saturate(1.2);
  transition: background-image 0.6s ease;
}
.amp-bg-overlay {
  position: absolute;
  inset: 0;
  background: linear-gradient(180deg, rgba(0,0,0,0.05) 0%, rgba(0,0,0,0.3) 40%, rgba(0,0,0,0.75) 100%);
}

// ─── Titlebar ────────────────────────────────────────────────────────────────
.amp-titlebar {
  position: relative;
  z-index: 10;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 16px 18px 0;
  flex-shrink: 0;
}
.amp-wbtn {
  width: 13px;
  height: 13px;
  border-radius: 50%;
  cursor: pointer;
  transition: opacity 0.15s;

  &:hover { opacity: 0.8; }

  &-close { background: #ff5f57; }
  &-min   { background: #febc2e; }
  &-max   { background: #28c840; }
}

// ─── Main layout ─────────────────────────────────────────────────────────────
.amp-main {
  position: relative;
  z-index: 5;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
}

// ─── Stage: cover / lyric switching area ─────────────────────────────────────
.amp-stage {
  flex: 1;
  min-height: 0;
  width: 100%;
  max-width: 420px;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  overflow: hidden;
}
.amp-stage-cover {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}
.amp-stage-lyric {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
}

// ─── Bottom controls (always visible) ────────────────────────────────────────
.amp-bottom {
  width: 100%;
  max-width: 420px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 0 32px 20px;
  gap: 0;
}

// ─── Cover ───────────────────────────────────────────────────────────────────
.amp-cover {
  width: min(240px, 55vw, 32vh);
  height: min(240px, 55vw, 32vh);
  border-radius: 14px;
  overflow: hidden;
  box-shadow: 0 8px 40px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.06);
  flex-shrink: 0;
  transition: transform 0.4s cubic-bezier(0.34, 1.56, 0.64, 1), box-shadow 0.4s ease;

  &.playing {
    transform: scale(1.04);
    box-shadow: 0 16px 60px rgba(0,0,0,0.6), 0 0 0 1px rgba(255,255,255,0.08);
  }
}
.amp-cover-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.amp-cover-fb {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #1c1c2e, #2d1b33);
  color: rgba(255,255,255,0.15);
}

// ─── Meta ────────────────────────────────────────────────────────────────────
.amp-meta {
  width: 100%;
  margin-top: 18px;
  margin-bottom: 16px;
}
.amp-meta-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}
.amp-meta-info {
  flex: 1;
  min-width: 0;
}
.amp-title {
  font-size: 20px;
  font-weight: 700;
  line-height: 1.25;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: #fff;
  letter-spacing: -0.3px;
}
.amp-artist {
  font-size: 14px;
  color: rgba(255,255,255,0.5);
  margin-top: 3px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.amp-album {
  color: rgba(255,255,255,0.35);
}
.amp-status {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 8px;
  flex-wrap: wrap;
}
.amp-tag {
  font-size: 10px;
  padding: 2px 8px;
  border-radius: 20px;
  background: rgba(255,255,255,0.08);
  color: rgba(255,255,255,0.45);
  font-weight: 500;
  font-variant-numeric: tabular-nums;
}
.amp-err  { font-size: 11px; color: #ff5e57; }
.amp-hint { font-size: 11px; color: rgba(255,255,255,0.3); }

// ─── Icon buttons (fav, toolbar) ─────────────────────────────────────────────
.amp-icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  width: 36px;
  height: 36px;
  border-radius: 50%;
  border: none;
  background: transparent;
  color: rgba(255,255,255,0.45);
  cursor: pointer;
  transition: color 0.15s, background 0.15s;
  flex-shrink: 0;
  position: relative;

  &:hover { color: rgba(255,255,255,0.85); }
  &.active { color: #fc3c44; }
  &:disabled { opacity: 0.3; pointer-events: none; }
}
.amp-fav-btn {
  margin-top: 2px;
  &.active { color: #fc3c44; }
}

// ─── Progress ────────────────────────────────────────────────────────────────
.amp-progress {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  margin-bottom: 16px;
}
.amp-time {
  font-size: 11px;
  color: rgba(255,255,255,0.4);
  font-variant-numeric: tabular-nums;
  font-weight: 500;
  min-width: 32px;
  text-align: center;
  flex-shrink: 0;
}
.amp-bar {
  flex: 1;
  height: 20px;
  display: flex;
  align-items: center;
  cursor: pointer;
  position: relative;

  &:hover .amp-bar-fill {
    background: rgba(255,255,255,0.9);
  }
  &:hover .amp-bar-thumb {
    transform: translateX(-50%) scale(1);
  }
}
.amp-bar-track {
  width: 100%;
  height: 3px;
  background: rgba(255,255,255,0.12);
  border-radius: 2px;
  overflow: hidden;
  position: relative;
}
.amp-bar-fill {
  height: 100%;
  background: rgba(255,255,255,0.7);
  border-radius: 2px;
  transition: background 0.15s;
}
.amp-bar-thumb {
  position: absolute;
  top: 50%;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 1px 6px rgba(0,0,0,0.4);
  transform: translateX(-50%) scale(0);
  transition: transform 0.12s;
  pointer-events: none;
  margin-top: -6px;
}

// ─── Controls ────────────────────────────────────────────────────────────────
.amp-controls {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  width: 100%;
  margin-bottom: 14px;
}
.amp-ctrl-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border-radius: 50%;
  border: none;
  background: transparent;
  color: rgba(255,255,255,0.6);
  cursor: pointer;
  transition: color 0.15s;
  flex-shrink: 0;

  &:hover { color: #fff; }
  &.active { color: #fc3c44; }
}
.amp-play-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 56px;
  height: 56px;
  border-radius: 50%;
  border: none;
  background: #fff;
  color: #000;
  cursor: pointer;
  flex-shrink: 0;
  transition: transform 0.15s, box-shadow 0.15s;
  box-shadow: 0 4px 16px rgba(0,0,0,0.3);
  margin: 0 8px;

  &:hover  { transform: scale(1.06); box-shadow: 0 6px 24px rgba(0,0,0,0.4); }
  &:active { transform: scale(0.95); }
}
.amp-vol-wrap {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-left: 4px;

  .amp-bar { width: 72px; flex: none; }
}

// ─── Toolbar ─────────────────────────────────────────────────────────────────
.amp-toolbar {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
}
.amp-sleep-badge {
  position: absolute;
  bottom: -2px;
  right: -2px;
  font-size: 9px;
  background: #fc3c44;
  color: #fff;
  border-radius: 6px;
  padding: 0 3px;
  font-variant-numeric: tabular-nums;
  pointer-events: none;
}
.amp-sleep-menu { padding: 4px 0; min-width: 140px; }
.amp-sleep-title { font-size: 12px; font-weight: 600; padding: 6px 14px 8px; color: rgba(0,0,0,0.45); }
.amp-sleep-item {
  display: block; width: 100%; text-align: left; padding: 7px 16px;
  border: none; background: transparent; font-size: 13px; color: rgba(0,0,0,0.8);
  cursor: pointer; font-family: inherit;
  &:hover { background: rgba(0,0,0,0.05); }
  &.on { color: #fc3c44; font-weight: 600; }
}

// ─── Lyric in stage ──────────────────────────────────────────────────────────
.amp-lyric-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: rgba(255,255,255,0.2);
  font-size: 14px;

  small {
    max-width: 82%;
    line-height: 1.45;
    text-align: center;
    color: rgba(255,255,255,0.38);
    user-select: text;
  }
}
.amp-lyric-scroll {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;

  &::-webkit-scrollbar { width: 3px; }
  &::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.06); border-radius: 2px; }
}
.amp-lyric-spacer { height: 45%; min-height: 60px; }
.amp-lyric-line {
  padding: 8px 16px;
  font-size: 16px;
  font-weight: 500;
  line-height: 1.55;
  color: rgba(255,255,255,0.22);
  cursor: pointer;
  transition: color 0.3s, font-size 0.25s;
  text-align: center;

  &:hover { color: rgba(255,255,255,0.5); }
  &.active {
    color: #fff;
    font-size: 19px;
    font-weight: 700;
  }
  &.past { color: rgba(255,255,255,0.1); }
}

// ─── Drawer ───────────────────────────────────────────────────────────────────
.amp-drawer {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  z-index: 20;
  max-height: 60%;
  display: flex;
  flex-direction: column;
  background: rgba(18,18,24,0.97);
  backdrop-filter: blur(20px);
  border-radius: 20px 20px 0 0;
  box-shadow: 0 -4px 30px rgba(0,0,0,0.5);
}
.amp-drawer-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  border: none;
  background: rgba(255,255,255,0.07);
  color: rgba(255,255,255,0.5);
  cursor: pointer;
  flex-shrink: 0;
  margin-left: 6px;
  transition: background 0.15s, color 0.15s;

  &:hover { background: rgba(255,255,255,0.12); color: #fff; }
}
.amp-drawer-head {
  padding: 14px 16px 0;
  flex-shrink: 0;
  border-bottom: 1px solid rgba(255,255,255,0.05);
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.amp-drawer-tabs {
  display: flex;
  align-items: center;
  gap: 2px;
  flex: 1;
  margin-bottom: 12px;
}
.amp-dtab {
  padding: 6px 14px;
  font-size: 12px;
  font-weight: 500;
  color: rgba(255,255,255,0.4);
  background: transparent;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  font-family: inherit;
  transition: color 0.15s, background 0.15s;

  &:hover { color: rgba(255,255,255,0.75); }
  &.on { color: #fff; background: rgba(255,255,255,0.07); }
}
.amp-drawer-search {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  background: rgba(255,255,255,0.05);
  border-radius: 8px;
  color: rgba(255,255,255,0.3);
  min-width: 100px;
  margin-bottom: 12px;
}
.amp-drawer-input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: #fff;
  font-size: 12px;
  font-family: inherit;
  &::placeholder { color: rgba(255,255,255,0.25); }
}
.amp-drawer-clear {
  background: transparent;
  border: none;
  color: rgba(255,255,255,0.35);
  cursor: pointer;
  display: flex;
  align-items: center;
  padding: 0;
}
.amp-drawer-list {
  flex: 1;
  overflow-y: auto;
  padding: 6px 0;
  &::-webkit-scrollbar { width: 3px; }
  &::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.05); border-radius: 2px; }
}
.amp-drow {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 16px;
  cursor: pointer;
  color: rgba(255,255,255,0.75);
  transition: background 0.1s;

  &:hover { background: rgba(255,255,255,0.03); }
  &.cur { background: rgba(252,60,68,0.08); color: #fff; }
}
.amp-dnum {
  width: 22px;
  text-align: center;
  font-size: 12px;
  color: rgba(255,255,255,0.3);
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
}
.amp-dthumb {
  width: 38px;
  height: 38px;
  border-radius: 6px;
  object-fit: cover;
  flex-shrink: 0;
  background: rgba(255,255,255,0.04);
}
.amp-dthumb-fb {
  display: flex;
  align-items: center;
  justify-content: center;
  color: rgba(255,255,255,0.15);
}
.amp-dmeta {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.amp-dname {
  font-size: 13px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.amp-dext {
  font-size: 10px;
  color: rgba(255,255,255,0.25);
}
.amp-dfav {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border-radius: 50%;
  border: none;
  background: transparent;
  color: rgba(255,255,255,0.2);
  cursor: pointer;
  flex-shrink: 0;
  transition: color 0.15s;

  &.liked, &:hover { color: #fc3c44; }
}
.amp-dempty {
  padding: 32px 16px;
  text-align: center;
  font-size: 12px;
  color: rgba(255,255,255,0.2);
}
.amp-dclear {
  display: block;
  margin: 6px auto 10px;
  padding: 5px 16px;
  border-radius: 8px;
  border: 1px solid rgba(255,255,255,0.08);
  background: transparent;
  color: rgba(255,255,255,0.35);
  font-size: 11px;
  cursor: pointer;
  font-family: inherit;
  transition: color 0.15s;

  &:hover { color: #fc3c44; border-color: rgba(252,60,68,0.3); }
}
</style>
