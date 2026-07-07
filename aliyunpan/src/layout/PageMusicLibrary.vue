<script setup lang='ts'>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import type { CSSProperties } from 'vue'
import { ArrowLeft, House, ListMusic, Mic2, Disc3, Folder, Heart, Music, Play, User, RefreshCw, X, PanelRightOpen, PanelRightClose, GripVertical, Search } from 'lucide-vue-next'
import useMusicLibraryStore, { type MusicSubTab } from '../store/musiclibrary'
import useSettingStore from '../setting/settingstore'
import { IMusicTrack } from '../types/music'
import { IPageMusic, IPageMusicTrack } from '../store/appstore'
import { useAppStore } from '../store'
import MusicScanner from '../utils/musicScanner'
import { enrichMusicLibrary } from '../utils/musicEnrichment'
import message from '../utils/message'
import PageMusic from './PageMusic.vue'
import useMusicPlayerStore, { type MusicPlayerState } from '../store/musicplayerstore'
import UserDAL from '../user/userdal'
import type { ITokenInfo } from '../user/userstore'

const musicStore = useMusicLibraryStore()
const settingStore = useSettingStore()
const appStore = useAppStore()
const musicPlayerStore = useMusicPlayerStore()

type LucideIconComponent = typeof House
const subTabs: { key: MusicSubTab; label: string; icon: LucideIconComponent }[] = [
  { key: 'home', label: '首页', icon: House },
  { key: 'all', label: '歌曲', icon: ListMusic },
  { key: 'artists', label: '艺人', icon: Mic2 },
  { key: 'albums', label: '专辑', icon: Disc3 },
  { key: 'folders', label: '文件夹', icon: Folder },
  { key: 'fav', label: '收藏', icon: Heart }
]

const searchQuery = ref('')
const showInlinePlayer = ref(false)
const groupDetail = ref<{ type: 'artist' | 'album' | 'folder'; title: string; items: IMusicTrack[] } | null>(null)
const userLabelMap = ref<Record<string, string>>({})
const scanAccounts = ref<ITokenInfo[]>([])
const selectedScanUserIds = ref<string[]>([])
type MusicFolderGroup = ReturnType<typeof useMusicLibraryStore>['byFolder'][number]
const selectedFolderKeys = ref<string[]>([])
const folderContextVisible = ref(false)
const folderContextPosition = ref({ x: 0, y: 0 })
const folderContextGroup = ref<MusicFolderGroup | null>(null)

const PANEL_WIDTH_KEY = 'pageMusicLibrary.playerWidth'
const PANEL_MIN_W = 280
const PANEL_MAX_W = 720
const PANEL_DEFAULT_W = 360
function loadPanelWidth(): number {
  const v = parseInt(localStorage.getItem(PANEL_WIDTH_KEY) || '', 10)
  if (!Number.isFinite(v)) return PANEL_DEFAULT_W
  return Math.min(Math.max(v, PANEL_MIN_W), PANEL_MAX_W)
}
const playerWidth = ref<number>(loadPanelWidth())
const playerPanelVisible = computed(() => showInlinePlayer.value && musicPlayerStore.panelVisible)

let dragStartX = 0
let dragStartW = 0
let dragging = false
function onResizeMouseDown(e: MouseEvent) {
  e.preventDefault()
  dragging = true
  dragStartX = e.clientX
  dragStartW = playerWidth.value
  document.addEventListener('mousemove', onResizeMouseMove)
  document.addEventListener('mouseup', onResizeMouseUp)
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
}
function onResizeMouseMove(e: MouseEvent) {
  if (!dragging) return
  const dx = dragStartX - e.clientX
  playerWidth.value = Math.min(Math.max(dragStartW + dx, PANEL_MIN_W), PANEL_MAX_W)
}
function onResizeMouseUp() {
  if (!dragging) return
  dragging = false
  document.removeEventListener('mousemove', onResizeMouseMove)
  document.removeEventListener('mouseup', onResizeMouseUp)
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  try { localStorage.setItem(PANEL_WIDTH_KEY, String(playerWidth.value)) } catch {}
}
function togglePlayerPanel() {
  if (musicPlayerStore.panelVisible) {
    musicPlayerStore.hidePanel()
  } else {
    musicPlayerStore.showPanel()
  }
}

onBeforeUnmount(() => {
  if (dragging) onResizeMouseUp()
})

const filteredAll = computed<IMusicTrack[]>(() => {
  const q = searchQuery.value.trim().toLowerCase()
  if (!q) return musicStore.tracks
  return musicStore.tracks.filter((t) => {
    return (
      (t.file_name || '').toLowerCase().includes(q) ||
      (t.title || '').toLowerCase().includes(q) ||
      (t.artist || '').toLowerCase().includes(q) ||
      (t.album || '').toLowerCase().includes(q)
    )
  })
})

const folderContextStyle = computed<CSSProperties>(() => ({
  position: 'fixed',
  left: `${folderContextPosition.value.x}px`,
  top: `${folderContextPosition.value.y}px`,
  zIndex: 9999,
  opacity: folderContextVisible.value ? 1 : 0
}))

const folderContextSelected = computed(() => {
  const group = folderContextGroup.value
  return !!group && selectedFolderKeys.value.includes(group.key)
})

const selectedFolderGroups = computed(() => {
  const keys = new Set(selectedFolderKeys.value)
  return musicStore.byFolder.filter((g) => keys.has(g.key))
})

const scanAccountOptions = computed(() => {
  return scanAccounts.value
    .filter((u) => !!u?.user_id && !!u?.access_token)
    .map((u) => ({ value: u.user_id, label: scanAccountLabel(u) }))
})

function formatTime(ts: number): string {
  if (!ts) return '从未扫描'
  const d = new Date(ts)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

function trackToPlaylist(t: IMusicTrack): IPageMusicTrack {
  return {
    user_id: t.user_id,
    drive_id: t.drive_id,
    file_id: t.file_id,
    parent_file_id: t.parent_file_id,
    file_name: t.file_name,
    ext: t.ext,
    size: t.size,
    category: t.category,
    icon: '',
    thumbnail: t.thumbnail,
    description: t.description,
    encType: t.encType,
    password: ''
  }
}

function playFromList(list: IMusicTrack[], target: IMusicTrack) {
  if (!list.length) return
  const playlist = list.map(trackToPlaylist)
  const idx = list.findIndex((x) => x.id === target.id)
  if (idx > 0) {
    const head = playlist.splice(idx, 1)[0]
    playlist.unshift(head)
  }
  const pageMusic: IPageMusic = {
    user_id: target.user_id,
    drive_id: target.drive_id,
    file_id: target.file_id,
    parent_file_id: target.parent_file_id,
    parent_file_name: target.parent_path || '',
    file_name: target.file_name,
    encType: target.encType || '',
    password: '',
    playlist
  }
  appStore.pageMusic = pageMusic
  showInlinePlayer.value = true
  musicPlayerStore.loadMusic(pageMusic)
  musicPlayerStore.showPanel()
}

function handlePlayerState(state: MusicPlayerState) {
  musicPlayerStore.updateState(state)
}

function openGroupDetail(type: 'artist' | 'album' | 'folder', title: string, items: IMusicTrack[]) {
  groupDetail.value = { type, title, items }
}

function groupTypeLabel(type: 'artist' | 'album' | 'folder'): string {
  if (type === 'artist') return '艺人'
  if (type === 'album') return '专辑'
  return '文件夹'
}

function driveLabel(driveId: string, userId = ''): string {
  if (driveId === 'cloud123') return '123 网盘'
  if (driveId === 'drive115') return '115 网盘'
  if (driveId === 'cloud139') return '139 云盘'
  if (driveId === 'cloud189') return '天翼云盘'
  if (driveId === 'baidu') return '百度网盘'
  if (driveId === 'pikpak') return 'PikPak'
  if (driveId === 'quark') return '夸克网盘'
  if (driveId === 'dropbox') return 'Dropbox'
  if (driveId === 'onedrive') return 'OneDrive'
  if (driveId === 'box') return 'Box'
  if (driveId.includes('resource')) return '阿里云盘资源盘'
  if (driveId.includes('backup')) return '阿里云盘备份盘'
  if (userId.startsWith('aliyun_') || driveId) return '阿里云盘'
  return '未知网盘'
}

function driveLabelFromToken(token: ITokenInfo): string {
  if (token.tokenfrom === 'cloud123') return '123 网盘'
  if (token.tokenfrom === '115') return '115 网盘'
  if (token.tokenfrom === '139') return '139 云盘'
  if (token.tokenfrom === '189') return '天翼云盘'
  if (token.tokenfrom === 'baidu') return '百度网盘'
  if (token.tokenfrom === 'pikpak') return 'PikPak'
  if (token.tokenfrom === 'quark') return '夸克网盘'
  if (token.tokenfrom === 'dropbox') return 'Dropbox'
  if (token.tokenfrom === 'onedrive') return 'OneDrive'
  if (token.tokenfrom === 'box') return 'Box'
  return '阿里云盘'
}

function scanAccountLabel(token: ITokenInfo): string {
  return `${driveLabelFromToken(token)} · ${token.nick_name || token.user_name || token.name || token.user_id}`
}

function userLabel(userId: string): string {
  return userLabelMap.value[userId] || userId || '未知账号'
}

function sourceLabel(t?: IMusicTrack): string {
  if (!t) return ''
  return `${driveLabel(t.drive_id, t.user_id)} · ${userLabel(t.user_id)}`
}

function folderSourceLabel(g: { user_id?: string; drive_id?: string }): string {
  return `${driveLabel(g.drive_id || '', g.user_id || '')} · ${userLabel(g.user_id || '')}`
}

function toggleFolderSelected(g: MusicFolderGroup) {
  const set = new Set(selectedFolderKeys.value)
  if (set.has(g.key)) set.delete(g.key)
  else set.add(g.key)
  selectedFolderKeys.value = Array.from(set)
}

function openFolderCard(g: MusicFolderGroup) {
  if (selectedFolderKeys.value.length > 0) {
    toggleFolderSelected(g)
    return
  }
  openGroupDetail('folder', g.name, g.items)
}

function openFolderContextMenu(event: MouseEvent, g: MusicFolderGroup) {
  event.preventDefault()
  event.stopPropagation()
  folderContextGroup.value = g
  folderContextPosition.value = { x: event.clientX, y: event.clientY }
  folderContextVisible.value = true
}

function closeFolderContextMenu() {
  folderContextVisible.value = false
  folderContextGroup.value = null
}

async function deleteFolderGroups(groups: MusicFolderGroup[]) {
  const uniq = new Set<string>()
  let trackCount = 0
  for (const g of groups) {
    trackCount += g.items.length
    for (const t of g.items) uniq.add(t.id)
  }
  if (!uniq.size) return
  const groupCount = groups.length
  const ok = window.confirm(`确定删除 ${groupCount} 个文件夹的音乐库记录？将移除 ${trackCount} 首已刮削音乐，不会删除网盘文件。`)
  if (!ok) return
  await musicStore.deleteTracksByIds(Array.from(uniq))
  selectedFolderKeys.value = selectedFolderKeys.value.filter((key) => musicStore.byFolder.some((g) => g.key === key))
  if (groupDetail.value?.type === 'folder' && groups.some((g) => g.items.some((t) => groupDetail.value?.items.some((gt) => gt.id === t.id)))) {
    groupDetail.value = null
  }
  message.success(`已删除 ${trackCount} 首音乐库记录`)
}

function deleteCurrentFolderFromMenu() {
  const g = folderContextGroup.value
  closeFolderContextMenu()
  if (!g) return
  deleteFolderGroups([g])
}

function deleteSelectedFoldersFromMenu() {
  const groups = selectedFolderGroups.value
  closeFolderContextMenu()
  if (!groups.length) return
  deleteFolderGroups(groups)
}

function toggleContextFolderSelected() {
  const g = folderContextGroup.value
  if (g) toggleFolderSelected(g)
  closeFolderContextMenu()
}

function closeGroupDetail() {
  groupDetail.value = null
}

function toggleScanUser(userId: string) {
  const idx = selectedScanUserIds.value.indexOf(userId)
  if (idx === -1) selectedScanUserIds.value = [...selectedScanUserIds.value, userId]
  else selectedScanUserIds.value = selectedScanUserIds.value.filter((id) => id !== userId)
}

async function startScan() {
  if (musicStore.isScanning) {
    message.info('扫描进行中…')
    return
  }
  if (!selectedScanUserIds.value.length) {
    message.warning('请选择要扫描的网盘')
    return
  }
  message.info('开始扫描，将在后台进行')
  try {
    await MusicScanner.getInstance().scanAllUsers({
      force: false,
      userIdAllowList: new Set(selectedScanUserIds.value)
    })
    message.success(`扫描完成，共 ${musicStore.totalCount} 首`)
    scheduleEnrich(800)
  } catch (e) {
    message.error('扫描失败：' + (e as Error).message)
  }
}

function stopScan() {
  MusicScanner.getInstance().stopScan()
}

async function clearLibrary() {
  await musicStore.clearAll()
  message.success('音乐库已清空')
}

function selectTab(t: MusicSubTab) {
  musicStore.setSubTab(t)
  scheduleEnrich()
}

function artistCover(g: { items: IMusicTrack[] }): string {
  return coverSource(artistCoverTrack(g))
}

function artistCoverTrack(g: { items: IMusicTrack[] }): IMusicTrack | undefined {
  for (const t of g.items) {
    if (coverSource(t)) return t
  }
  return undefined
}

const coverDebugSeen = new Set<string>()
const COVER_DEBUG_LIMIT = 20

function coverSource(t?: IMusicTrack): string {
  if (!t) return ''
  return t.cover_url || t.thumbnail || ''
}

function coverSourceType(t?: IMusicTrack): 'cover_url' | 'thumbnail' | 'empty' {
  if (!t) return 'empty'
  if (t.cover_url) return 'cover_url'
  if (t.thumbnail) return 'thumbnail'
  return 'empty'
}

function debugCoverLoad(t: IMusicTrack, area: string, event: Event) {
  const img = event.target as HTMLImageElement | null
  if (!img) return
  const key = `${area}:${t.id}:${img.currentSrc || img.src}`
  if (coverDebugSeen.has(key) || coverDebugSeen.size >= COVER_DEBUG_LIMIT) return
  coverDebugSeen.add(key)
  const rect = img.getBoundingClientRect()
  const style = window.getComputedStyle(img)
}

function debugCoverError(t: IMusicTrack, area: string, event: Event) {
  
}

let enrichTimer: number | undefined
let enrichLoopActive = false
async function runEnrichLoop() {
  if (enrichLoopActive) return
  enrichLoopActive = true
  try {
    while (musicStore.totalCount > 0) {
      const got = await enrichMusicLibrary(40)
      if (got <= 0) break
      await new Promise((r) => setTimeout(r, 1500))
    }
  } catch { /* ignore */ }
  finally {
    enrichLoopActive = false
  }
}
function scheduleEnrich(delayMs: number = 600) {
  if (enrichTimer) {
    window.clearTimeout(enrichTimer)
  }
  enrichTimer = window.setTimeout(() => {
    enrichTimer = undefined
    if (!musicStore.totalCount) return
    runEnrichLoop()
  }, delayMs) as unknown as number
}

function stripExt(name: string): string {
  const i = name.lastIndexOf('.')
  return i > 0 ? name.slice(0, i) : name
}

onMounted(async () => {
  await musicStore.loadFromDB()
  const users = await UserDAL.GetUserListFromDB().catch(() => [])
  scanAccounts.value = users.filter((u) => !!u?.user_id && !!u?.access_token)
  selectedScanUserIds.value = scanAccounts.value.map((u) => u.user_id)
  const map: Record<string, string> = {}
  for (const u of users) {
    if (!u?.user_id) continue
    map[u.user_id] = u.nick_name || u.user_name || u.name || u.user_id
  }
  userLabelMap.value = map
  scheduleEnrich(1500)
})
</script>

<template>
  <div class="aml">
    <div class="aml-main-row">
      <!-- Sidebar -->
      <aside class="aml-sidebar">
        <div class="aml-sidebar-brand">
          <Music :size="22" :stroke-width="1.5" class="aml-sidebar-brand-icon" />
          <div class="aml-sidebar-brand-text">
            <div class="aml-sidebar-brand-title">音乐</div>
            <div class="aml-sidebar-brand-sub">{{ musicStore.totalCount }} 首歌曲</div>
          </div>
        </div>

        <div v-for="tab in subTabs" :key="tab.key"
             :class="['aml-sidebar-item', musicStore.subTab === tab.key ? 'active' : '']"
             @click="selectTab(tab.key)">
          <component :is="tab.icon" :size="20" :stroke-width="1.5" class="aml-sidebar-icon" />
          <span>{{ tab.label }}</span>
        </div>

        <template v-if="!groupDetail">
          <div class="aml-sidebar-divider"></div>
          <div class="aml-sidebar-scan">
            <div class="aml-sidebar-scan-title">扫描</div>
            <a-dropdown trigger="click" :disabled="musicStore.isScanning">
              <a-button size="mini" long class="aml-drive-select-btn">
                {{ selectedScanUserIds.length === scanAccountOptions.length ? '全部网盘' : `${selectedScanUserIds.length} 个网盘` }}
              </a-button>
              <template #content>
                <div class="aml-drive-dropdown">
                  <label v-for="opt in scanAccountOptions" :key="opt.value" class="aml-drive-item" @click.stop>
                    <a-checkbox
                      :model-value="selectedScanUserIds.includes(opt.value)"
                      @change="toggleScanUser(opt.value)"
                    />
                    <span>{{ opt.label }}</span>
                  </label>
                  <div class="aml-drive-footer">
                    <a-link size="small" @click="selectedScanUserIds = scanAccountOptions.map(o => o.value)">全选</a-link>
                    <a-link size="small" @click="selectedScanUserIds = []">清空</a-link>
                  </div>
                </div>
              </template>
            </a-dropdown>
            <div class="aml-scan-btns">
              <a-button v-if="!musicStore.isScanning" type="primary" size="mini" long @click="startScan">
                开始扫描
              </a-button>
              <a-button v-else status="warning" size="mini" long @click="stopScan">停止扫描</a-button>
            </div>
            <a-button size="mini" long status="danger" @click="clearLibrary">清空资料库</a-button>
          </div>
        </template>

        <div class="aml-sidebar-footer">
          <div v-if="musicStore.isScanning" class="aml-sidebar-scanning">{{ musicStore.scanLabel || '扫描中…' }}</div>
          <div v-else-if="musicStore.lastScanAt" class="aml-sidebar-lastscan">{{ formatTime(musicStore.lastScanAt) }}</div>
        </div>
      </aside>

      <!-- Content Area -->
      <div class="aml-content-area">
        <!-- Header -->
        <div class="aml-header">
          <div class="aml-header-left">
            <Music :size="22" :stroke-width="1.5" class="aml-header-icon" />
            <span v-if="groupDetail" class="aml-back" @click="closeGroupDetail">
              <ArrowLeft :size="18" :stroke-width="1.5" />
            </span>
            <h2 class="aml-title">{{ groupDetail ? groupDetail.title : '音乐' }}</h2>
            <span v-if="groupDetail" class="aml-title-sub">{{ groupTypeLabel(groupDetail.type) }} · {{ groupDetail.items.length }} 首</span>
          </div>
          <div class="aml-header-right">
            <div class="aml-search-box">
              <Search :size="14" :stroke-width="1.5" class="aml-search-icon" />
              <input v-model="searchQuery" placeholder="搜索资料库…" class="aml-search-input" />
              <button v-if="searchQuery" class="aml-search-clear" @click="searchQuery = ''">✕</button>
            </div>
            <a-button
              v-if="!groupDetail"
              type="outline"
              size="small"
              :disabled="!showInlinePlayer"
              @click="togglePlayerPanel"
              :class="{ 'player-active': musicPlayerStore.panelVisible }"
            >
              <PanelRightOpen v-if="!musicPlayerStore.panelVisible" :size="14" :stroke-width="1.5" />
              <PanelRightClose v-else :size="14" :stroke-width="1.5" />
            </a-button>
            <a-button v-if="groupDetail" type="primary" size="small" @click="playFromList(groupDetail.items, groupDetail.items[0])">
              <Play :size="14" :stroke-width="2" /> 播放
            </a-button>
          </div>
        </div>

        <!-- Scanning progress -->
        <div v-if="musicStore.isScanning" class="aml-scan-bar">
          <div class="aml-scan-track">
            <div class="aml-scan-fill"></div>
          </div>
          <span class="aml-scan-text">
            {{ musicStore.scanLabel || '正在扫描…' }} · {{ musicStore.scanScanned }} 个文件 · 已收录 {{ musicStore.scanFound }} 首
          </span>
        </div>

        <!-- Content scroll -->
        <div class="aml-scroll-area">

          <!-- Group detail view -->
          <div v-if="groupDetail" class="aml-group-detail">
            <div class="aml-tracklist">
              <div
                v-for="(t, i) in groupDetail.items"
                :key="t.id"
                class="aml-track"
                @click="playFromList(groupDetail.items, t)"
              >
                <span class="aml-track-idx">{{ i + 1 }}</span>
                <div class="aml-track-cover">
                  <img v-if="t.cover_url || t.thumbnail" :src="coverSource(t)" alt="" @load="debugCoverLoad(t, 'group-detail', $event)" @error="debugCoverError(t, 'group-detail', $event)" />
                  <Music v-else :size="16" :stroke-width="1.5" />
                </div>
                <div class="aml-track-meta">
                  <div class="aml-track-title">{{ t.title || stripExt(t.file_name) }}</div>
                  <div class="aml-track-artist">{{ t.artist || '未知艺人' }}<span v-if="t.album"> — {{ t.album }}</span></div>
                </div>
                <div class="aml-track-source">{{ sourceLabel(t) }}</div>
              </div>
            </div>
          </div>

          <!-- Home page -->
          <div v-else-if="musicStore.subTab === 'home'" class="aml-home">
            <div v-if="!musicStore.totalCount" class="aml-empty-state">
              <Music :size="56" :stroke-width="1" class="aml-empty-icon" />
              <div class="aml-empty-title">资料库为空</div>
              <div class="aml-empty-sub">选择左侧网盘，点击「开始扫描」收录音乐</div>
            </div>
            <template v-else>
              <section class="aml-hscroll-section">
                <div class="aml-section-label">最近添加</div>
                <div class="aml-hscroll">
                  <div
                    v-for="t in musicStore.recentlyAdded"
                    :key="t.id"
                    class="aml-card-h"
                    @click="playFromList(musicStore.recentlyAdded, t)"
                  >
                    <div class="aml-card-h-cover">
                      <img v-if="t.cover_url || t.thumbnail" :src="coverSource(t)" alt="" @load="debugCoverLoad(t, 'recently-added', $event)" @error="debugCoverError(t, 'recently-added', $event)" />
                      <Music v-else :size="28" :stroke-width="1.5" />
                    </div>
                    <div class="aml-card-h-title">{{ t.title || stripExt(t.file_name) }}</div>
                    <div class="aml-card-h-sub">{{ t.artist || '未知艺人' }}</div>
                  </div>
                </div>
              </section>

              <section class="aml-section">
                <div class="aml-section-label">
                  推荐
                  <button class="aml-reroll-btn" @click="musicStore.rerollRandom()">
                    <RefreshCw :size="13" :stroke-width="1.5" />
                  </button>
                </div>
                <div class="aml-grid">
                  <div
                    v-for="t in musicStore.randomPicks"
                    :key="t.id"
                    class="aml-card"
                    @click="playFromList(musicStore.randomPicks, t)"
                  >
                    <div class="aml-card-cover">
                      <img v-if="t.cover_url || t.thumbnail" :src="coverSource(t)" alt="" @load="debugCoverLoad(t, 'random-picks', $event)" @error="debugCoverError(t, 'random-picks', $event)" />
                      <Music v-else :size="32" :stroke-width="1.5" />
                    </div>
                    <div class="aml-card-title">{{ t.title || stripExt(t.file_name) }}</div>
                    <div class="aml-card-sub">{{ t.artist || '未知艺人' }}</div>
                  </div>
                </div>
              </section>
            </template>
          </div>

          <!-- All songs -->
          <div v-else-if="musicStore.subTab === 'all'" class="aml-all-songs">
            <div v-if="!filteredAll.length" class="aml-empty-state">
              <ListMusic :size="48" :stroke-width="1" class="aml-empty-icon" />
              <div class="aml-empty-title">暂无歌曲</div>
            </div>
            <div v-else class="aml-tracklist">
              <div
                v-for="(t, i) in filteredAll"
                :key="t.id"
                class="aml-track"
                @click="playFromList(filteredAll, t)"
              >
                <span class="aml-track-idx">{{ i + 1 }}</span>
                <div class="aml-track-cover">
                  <img v-if="t.cover_url || t.thumbnail" :src="coverSource(t)" alt="" @load="debugCoverLoad(t, 'all-songs', $event)" @error="debugCoverError(t, 'all-songs', $event)" />
                  <Music v-else :size="16" :stroke-width="1.5" />
                </div>
                <div class="aml-track-meta">
                  <div class="aml-track-title">{{ t.title || stripExt(t.file_name) }}</div>
                  <div class="aml-track-artist">{{ t.artist || '未知艺人' }}</div>
                </div>
                <div class="aml-track-album">{{ t.album || '' }}</div>
              </div>
            </div>
          </div>

          <!-- Artists -->
          <div v-else-if="musicStore.subTab === 'artists'" class="aml-grid-view">
            <div v-if="!musicStore.byArtist.length" class="aml-empty-state">
              <Mic2 :size="48" :stroke-width="1" class="aml-empty-icon" />
              <div class="aml-empty-title">暂无艺人</div>
            </div>
            <div v-else class="aml-grid">
              <div
                v-for="g in musicStore.byArtist"
                :key="g.artist"
                class="aml-card"
                @click="openGroupDetail('artist', g.artist, g.items)"
              >
                <div class="aml-card-cover circle">
                  <img v-if="artistCoverTrack(g)" :src="artistCover(g)" alt="" @load="debugCoverLoad(artistCoverTrack(g)!, 'artist', $event)" @error="debugCoverError(artistCoverTrack(g)!, 'artist', $event)" />
                  <User v-else :size="32" :stroke-width="1.5" />
                </div>
                <div class="aml-card-title">{{ g.artist }}</div>
                <div class="aml-card-sub">{{ g.count }} 首</div>
              </div>
            </div>
          </div>

          <!-- Albums -->
          <div v-else-if="musicStore.subTab === 'albums'" class="aml-grid-view">
            <div v-if="!musicStore.byAlbum.length" class="aml-empty-state">
              <Disc3 :size="48" :stroke-width="1" class="aml-empty-icon" />
              <div class="aml-empty-title">暂无专辑</div>
            </div>
            <div v-else class="aml-grid">
              <div
                v-for="g in musicStore.byAlbum"
                :key="g.album"
                class="aml-card"
                @click="openGroupDetail('album', g.album, g.items)"
              >
                <div class="aml-card-cover">
                  <img v-if="g.items[0]?.cover_url || g.items[0]?.thumbnail" :src="coverSource(g.items[0])" alt="" @load="debugCoverLoad(g.items[0], 'album', $event)" @error="debugCoverError(g.items[0], 'album', $event)" />
                  <Disc3 v-else :size="32" :stroke-width="1.5" />
                </div>
                <div class="aml-card-title">{{ g.album }}</div>
                <div class="aml-card-sub">{{ g.count }} 首</div>
              </div>
            </div>
          </div>

          <!-- Folders -->
          <div v-else-if="musicStore.subTab === 'folders'" class="aml-grid-view">
            <div v-if="!musicStore.byFolder.length" class="aml-empty-state">
              <Folder :size="48" :stroke-width="1" class="aml-empty-icon" />
              <div class="aml-empty-title">暂无文件夹</div>
            </div>
            <div v-else class="aml-grid">
              <div
                v-for="g in musicStore.byFolder"
                :key="g.key"
                :class="['aml-card', selectedFolderKeys.includes(g.key) ? 'selected' : '']"
                @click="openFolderCard(g)"
                @contextmenu.prevent="openFolderContextMenu($event, g)"
              >
                <div class="aml-card-cover folder">
                  <div class="aml-folder-check" :class="{ checked: selectedFolderKeys.includes(g.key) }" @click.stop="toggleFolderSelected(g)">
                    <span v-if="selectedFolderKeys.includes(g.key)">✓</span>
                  </div>
                  <Folder :size="36" :stroke-width="1.5" />
                </div>
                <div class="aml-card-title">{{ g.name }}</div>
                <div class="aml-card-sub">{{ g.count }} 首 · {{ folderSourceLabel(g) }}</div>
              </div>
            </div>
          </div>

          <!-- Favorites -->
          <div v-else-if="musicStore.subTab === 'fav'" class="aml-fav-songs">
            <div v-if="!musicStore.favoritesTracks.length" class="aml-empty-state">
              <Heart :size="48" :stroke-width="1" class="aml-empty-icon" />
              <div class="aml-empty-title">暂无收藏</div>
              <div class="aml-empty-sub">在播放器页面点击心形收藏歌曲</div>
            </div>
            <div v-else class="aml-tracklist">
              <div
                v-for="(t, i) in musicStore.favoritesTracks"
                :key="t.id"
                class="aml-track"
                @click="playFromList(musicStore.favoritesTracks, t)"
              >
                <span class="aml-track-idx">{{ i + 1 }}</span>
                <div class="aml-track-cover">
                  <img v-if="t.cover_url || t.thumbnail" :src="coverSource(t)" alt="" @load="debugCoverLoad(t, 'favorites', $event)" @error="debugCoverError(t, 'favorites', $event)" />
                  <Music v-else :size="16" :stroke-width="1.5" />
                </div>
                <div class="aml-track-meta">
                  <div class="aml-track-title">{{ t.title || stripExt(t.file_name) }}</div>
                  <div class="aml-track-artist">{{ t.artist || '未知艺人' }}</div>
                </div>
              </div>
            </div>
          </div>

        </div>
      </div>

      <!-- Inline Player Panel -->
      <aside v-show="playerPanelVisible" class="aml-player-panel" :style="{ width: playerWidth + 'px' }">
        <div class="aml-player-resizer" title="拖动调整宽度" @mousedown="onResizeMouseDown">
          <GripVertical :size="14" :stroke-width="1.5" />
        </div>
        <div class="aml-player-inner">
          <div class="aml-side-player-inner">
            <PageMusic
              embedded
              side-panel
              @state-change="handlePlayerState"
            />
          </div>
        </div>
      </aside>

      <!-- Folder context menu -->
      <a-dropdown
        :popup-visible="folderContextVisible"
        :style="folderContextStyle"
        @popup-visible-change="closeFolderContextMenu"
      >
        <div style="width:1px;height:1px;visibility:hidden" />
        <template #content>
          <div class="aml-folder-menu">
            <button class="aml-folder-menu-item" @click="toggleContextFolderSelected">
              {{ folderContextSelected ? '取消选择' : '选择文件夹' }}
            </button>
            <button class="aml-folder-menu-item danger" @click="deleteCurrentFolderFromMenu">删除此文件夹记录</button>
            <button class="aml-folder-menu-item danger" :disabled="!selectedFolderGroups.length" @click="deleteSelectedFoldersFromMenu">
              删除已选文件夹（{{ selectedFolderGroups.length }}）
            </button>
          </div>
        </template>
      </a-dropdown>
    </div>
  </div>
</template>

<style scoped>
/* ===== Apple Music Library ===== */

.aml {
  height: 100%;
  overflow: hidden;
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Display", "Helvetica Neue", sans-serif;
  background: var(--color-bg-1);
  color: var(--color-text-1);
  -webkit-font-smoothing: antialiased;
}

.aml-main-row {
  display: flex;
  height: 100%;
  overflow: hidden;
}

/* --- Sidebar --- */

.aml-sidebar {
  flex: 0 0 220px;
  display: flex;
  flex-direction: column;
  padding: 18px 12px 0;
  background: var(--color-fill-1);
  border-right: 1px solid var(--color-border);
  overflow: hidden;
}

.aml-sidebar-brand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 4px 16px;
}

.aml-sidebar-brand-icon {
  color: #FF2D55;
  flex-shrink: 0;
}

.aml-sidebar-brand-text {
  min-width: 0;
}

.aml-sidebar-brand-title {
  font-size: 18px;
  font-weight: 700;
  color: var(--color-text-1);
}

.aml-sidebar-brand-sub {
  margin-top: 2px;
  font-size: 12px;
  color: var(--color-text-3);
}

.aml-sidebar-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 9px 12px;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 400;
  color: var(--color-text-2);
  cursor: pointer;
  transition: all 0.12s;

  &:hover {
    background: var(--color-fill-2);
    color: var(--color-text-1);
  }

  &.active {
    background: rgba(255,45,85,0.12);
    color: #FF2D55;
    font-weight: 500;
  }
}

.aml-sidebar-icon {
  flex-shrink: 0;
}

.aml-sidebar-divider {
  margin: 16px 8px;
  height: 1px;
  background: var(--color-border);
}

.aml-sidebar-scan {
  padding: 0 4px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.aml-sidebar-scan-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--color-text-3);
  text-transform: uppercase;
  letter-spacing: 0.6px;
}

.aml-drive-select-btn:deep(.arco-btn) {
  font-size: 12px;
  justify-content: flex-start;
}

.aml-drive-dropdown {
  min-width: 180px;
  max-height: 240px;
  overflow-y: auto;
  padding: 4px 0;
}

.aml-drive-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  cursor: pointer;
  font-size: 13px;
  color: var(--color-text-1);

  &:hover { background: var(--color-fill-2); }
}

.aml-drive-footer {
  display: flex;
  gap: 12px;
  padding: 6px 12px;
  border-top: 1px solid var(--color-border);
}

.aml-scan-btns {
  display: flex;
  gap: 4px;
}

.aml-sidebar-footer {
  margin-top: auto;
  padding: 14px 4px 16px;
}

.aml-sidebar-scanning {
  font-size: 11px;
  color: #FF2D55;
}

.aml-sidebar-lastscan {
  font-size: 11px;
  color: var(--color-text-4);
}

/* --- Content Area --- */

.aml-content-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
}

.aml-header {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 18px 28px 0;
  gap: 16px;
}

.aml-header-left {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.aml-header-icon {
  color: #FF2D55;
  flex-shrink: 0;
}

.aml-back {
  display: flex;
  align-items: center;
  padding: 4px;
  border-radius: 6px;
  cursor: pointer;
  color: var(--color-text-3);
  transition: all 0.15s;

  &:hover {
    color: var(--color-text-1);
    background: var(--color-fill-2);
  }
}

.aml-title {
  font-size: 22px;
  font-weight: 700;
  letter-spacing: -0.01em;
  margin: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.aml-title-sub {
  font-size: 13px;
  color: var(--color-text-3);
  white-space: nowrap;
}

.aml-header-right {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.aml-search-box {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: var(--color-fill-2);
  border-radius: 8px;
  width: 200px;
  transition: background 0.15s;

  &:focus-within {
    background: var(--color-fill-3);
    width: 260px;
  }
}

.aml-search-icon {
  color: var(--color-text-3);
  flex-shrink: 0;
}

.aml-search-input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: var(--color-text-1);
  font-size: 13px;
  font-family: inherit;
  min-width: 0;

  &::placeholder { color: var(--color-text-3); }
}

.aml-search-clear {
  background: transparent;
  border: none;
  color: var(--color-text-3);
  cursor: pointer;
  font-size: 12px;
  padding: 0;
  line-height: 1;

  &:hover { color: var(--color-text-1); }
}

/* Scan progress bar */

.aml-scan-bar {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 8px 28px 0;
}

.aml-scan-track {
  flex: 0 0 180px;
  height: 3px;
  background: var(--color-fill-2);
  border-radius: 2px;
  overflow: hidden;
}

.aml-scan-fill {
  height: 100%;
  width: 50%;
  background: linear-gradient(90deg, #FF2D55, #FF6B8A);
  background-size: 200% 100%;
  animation: aml-scan-pulse 1.5s ease-in-out infinite;
}

@keyframes aml-scan-pulse {
  0% { background-position: 0% 50%; }
  50% { background-position: 100% 50%; }
  100% { background-position: 0% 50%; }
}

.aml-scan-text {
  font-size: 11px;
  color: var(--color-text-3);
  font-weight: 500;
}

/* Scroll area */

.aml-scroll-area {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 20px 28px 24px;

  &::-webkit-scrollbar { width: 6px; }
  &::-webkit-scrollbar-thumb {
    background: var(--color-fill-3);
    border-radius: 3px;
  }
}

/* Sections */

.aml-section,
.aml-hscroll-section {
  margin-bottom: 36px;
}

.aml-section-label {
  font-size: 15px;
  font-weight: 600;
  color: var(--color-text-1);
  margin-bottom: 14px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.aml-reroll-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  border: none;
  background: var(--color-fill-2);
  color: var(--color-text-3);
  cursor: pointer;
  transition: all 0.15s;

  &:hover {
    background: var(--color-fill-3);
    color: var(--color-text-1);
  }
}

/* Horizontal scroll */

.aml-hscroll {
  display: flex;
  gap: 14px;
  overflow-x: auto;
  padding-bottom: 8px;

  &::-webkit-scrollbar { height: 4px; }
  &::-webkit-scrollbar-thumb { background: var(--color-fill-3); border-radius: 2px; }
}

.aml-card-h {
  flex: 0 0 140px;
  cursor: pointer;
  user-select: none;

  &:hover .aml-card-h-cover { transform: scale(1.02); }
}

.aml-card-h-cover {
  width: 140px;
  height: 140px;
  border-radius: 10px;
  overflow: hidden;
  background: var(--color-fill-2);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-4);
  transition: transform 0.2s;

  img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    object-position: center;
    display: block;
    background: var(--color-fill-2);
  }
}

.aml-card-h-cover > img,
.aml-card-cover > img,
.aml-track-cover > img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  object-position: center;
  display: block;
  background: var(--color-fill-2);
}

.aml-card-h-title {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-1);
  margin-top: 8px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 140px;
}

.aml-card-h-sub {
  font-size: 11px;
  color: var(--color-text-3);
  margin-top: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 140px;
}

/* Grid */

.aml-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 20px;
}

.aml-card {
  cursor: pointer;
  user-select: none;

  &.selected .aml-card-cover {
    outline: 2px solid #FF2D55;
    outline-offset: 2px;
  }

  &:hover .aml-card-cover { transform: scale(1.02); }
}

.aml-card-cover {
  aspect-ratio: 1;
  border-radius: 10px;
  overflow: hidden;
  background: var(--color-fill-2);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-4);
  transition: transform 0.2s;
  position: relative;

  &.circle { border-radius: 50%; }
  &.folder { background: linear-gradient(135deg, rgba(255,45,85,0.1), rgba(255,45,85,0.05)); }

  img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    object-position: center;
    display: block;
    background: var(--color-fill-2);
  }
}

.aml-card-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-1);
  margin-top: 9px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.aml-card-sub {
  font-size: 12px;
  color: var(--color-text-3);
  margin-top: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.aml-folder-check {
  position: absolute;
  top: 6px;
  right: 6px;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  border: 2px solid var(--color-border);
  background: transparent;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  color: transparent;
  cursor: pointer;
  transition: all 0.15s;
  z-index: 2;

  &.checked {
    border-color: #FF2D55;
    background: #FF2D55;
    color: #fff;
  }
}

/* Track list */

.aml-tracklist {
  display: flex;
  flex-direction: column;
  gap: 0px;
}

.aml-track {
  display: grid;
  grid-template-columns: 32px 44px 1fr auto;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.1s;
  color: var(--color-text-1);

  &:hover { background: var(--color-fill-2); }
}

.aml-track-idx {
  font-size: 13px;
  color: var(--color-text-3);
  font-variant-numeric: tabular-nums;
  text-align: center;
  width: 32px;
  font-weight: 500;
}

.aml-track-cover {
  width: 44px;
  height: 44px;
  border-radius: 6px;
  overflow: hidden;
  background: var(--color-fill-2);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-4);
  flex-shrink: 0;

  img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    object-position: center;
    display: block;
    background: var(--color-fill-2);
  }
}

.aml-track-meta {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.aml-track-title {
  font-size: 14px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.aml-track-artist {
  font-size: 12px;
  color: var(--color-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.aml-track-album,
.aml-track-source {
  font-size: 12px;
  color: var(--color-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 180px;
  text-align: right;
}

/* Empty state */

.aml-empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 80px 24px;
  text-align: center;
}

.aml-empty-icon {
  color: var(--color-text-4);
  margin-bottom: 16px;
}

.aml-empty-title {
  font-size: 17px;
  font-weight: 600;
  color: var(--color-text-2);
}

.aml-empty-sub {
  font-size: 13px;
  color: var(--color-text-4);
  margin-top: 6px;
}

/* Player panel */

.aml-player-panel {
  flex: 0 0 auto;
  height: 100%;
  border-left: 1px solid var(--color-border);
  overflow: hidden;
}

.aml-player-resizer {
  position: absolute;
  top: 0;
  left: 0;
  width: 8px;
  height: 100%;
  cursor: col-resize;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-4);
  z-index: 2;
  transition: background 0.15s, color 0.15s;

  &:hover {
    background: var(--color-fill-2);
    color: var(--color-text-2);
  }
}

.aml-player-inner {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.aml-side-player-inner {
  height: 100%;
  overflow: hidden;
  border-radius: 10px;
  border: 1px solid var(--color-border);
  background: var(--color-bg-2);
}

.aml-player-empty {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 24px;
  text-align: center;
  color: var(--color-text-4);
}

.aml-player-empty-title {
  margin-top: 14px;
  font-size: 15px;
  font-weight: 600;
  color: var(--color-text-3);
}

.aml-player-empty-sub {
  margin-top: 4px;
  font-size: 12px;
  color: var(--color-text-4);
}

/* Folder context menu */

.aml-folder-menu {
  padding: 4px 0;
}

.aml-folder-menu-item {
  display: block;
  width: 100%;
  text-align: left;
  padding: 8px 16px;
  border: none;
  background: transparent;
  font-size: 13px;
  color: var(--color-text-1);
  cursor: pointer;
  font-family: inherit;

  &:hover { background: var(--color-fill-2); }
  &.danger { color: #FF2D55; }
  &:disabled { opacity: 0.3; cursor: default; }
}

/* btn tweak */
.player-active:deep(.arco-btn) {
  border-color: #FF2D55;
  color: #FF2D55;
}
</style>
