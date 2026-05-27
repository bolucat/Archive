<script setup lang='ts'>
import { computed, onMounted, ref } from 'vue'
import { House, ListMusic, Mic2, Disc3, Folder, Heart, Music, User, RefreshCw, X } from 'lucide-vue-next'
import useMusicLibraryStore, { type MusicSubTab } from '../store/musiclibrary'
import useSettingStore from '../setting/settingstore'
import { IMusicTrack } from '../types/music'
import { IPageMusic, IPageMusicTrack } from '../store/appstore'
import MusicScanner from '../utils/musicScanner'
import { enrichMusicLibrary } from '../utils/musicEnrichment'
import message from '../utils/message'

const musicStore = useMusicLibraryStore()
const settingStore = useSettingStore()

type LucideIconComponent = typeof House
const subTabs: { key: MusicSubTab; label: string; icon: LucideIconComponent }[] = [
  { key: 'home', label: '首页', icon: House },
  { key: 'all', label: '全部', icon: ListMusic },
  { key: 'artists', label: '艺人', icon: Mic2 },
  { key: 'albums', label: '专辑', icon: Disc3 },
  { key: 'folders', label: '文件夹', icon: Folder },
  { key: 'fav', label: '收藏', icon: Heart }
]

const searchQuery = ref('')

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
  if (typeof window !== 'undefined' && (window as any).WebOpenWindow) {
    ;(window as any).WebOpenWindow({ page: 'PageMusic', data: pageMusic, theme: 'dark' })
  }
}

async function startScan() {
  if (musicStore.isScanning) {
    message.info('扫描进行中…')
    return
  }
  message.info('开始扫描，将在后台进行')
  try {
    await MusicScanner.getInstance().scanAllUsers(false)
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
  for (const t of g.items) {
    const c = t.cover_url || t.thumbnail
    if (c) return c
  }
  return ''
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
      // 让 UI 有缓冲
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

onMounted(async () => {
  await musicStore.loadFromDB()
  scheduleEnrich(1500)
})
</script>

<template>
  <div class='music-library'>
    <div class='ml-header'>
      <div class='ml-title-row'>
        <div class='ml-title'>音乐</div>
        <div class='ml-subtitle'>
          共 {{ musicStore.totalCount }} 首 · 上次扫描：{{ formatTime(musicStore.lastScanAt) }}
        </div>
        <div class='flexauto'></div>
        <a-input-search
          v-model='searchQuery'
          placeholder='搜索歌曲 / 艺人 / 专辑'
          allow-clear
          style='width: 280px; margin-right: 12px'
        />
        <a-button v-if='!musicStore.isScanning' type='primary' @click='startScan'>
          <template #icon><RefreshCw :size='14' :stroke-width='1.8' /></template>
          扫描
        </a-button>
        <a-button v-else status='warning' @click='stopScan'>
          <template #icon><X :size='14' :stroke-width='1.8' /></template>
          停止
        </a-button>
        <a-tooltip content='每次启动 App 静默扫描音频；间隔由设置控制' position='bottom'>
          <a-switch
            :model-value='settingStore.uiLibraryAutoScanMusic'
            @update:model-value='settingStore.updateStore({ uiLibraryAutoScanMusic: Boolean($event) })'
            checked-text='自动'
            unchecked-text='手动'
            style='margin-left: 8px'
          />
        </a-tooltip>
        <a-popconfirm content='确定清空整个音乐库？此操作不可恢复' @ok='clearLibrary'>
          <a-button status='danger' style='margin-left: 8px'>清空</a-button>
        </a-popconfirm>
      </div>
      <div v-if='musicStore.isScanning' class='ml-progress'>
        <div class='ml-progress-bar'>
          <div class='ml-progress-fill'></div>
        </div>
        <div class='ml-progress-text'>
          {{ musicStore.scanLabel || '正在扫描…' }} · 已检查 {{ musicStore.scanScanned }} 个文件 · 已收录 {{ musicStore.scanFound }} 首
        </div>
      </div>
    </div>

    <div class='ml-body'>
      <aside class='ml-side-nav'>
        <div
          v-for='tab in subTabs'
          :key='tab.key'
          class='ml-nav-item'
          :class="{ active: musicStore.subTab === tab.key }"
          @click='selectTab(tab.key)'
        >
          <component :is='tab.icon' :size='18' :stroke-width='1.8' class='ml-nav-icon' />
          <span class='ml-nav-label'>{{ tab.label }}</span>
        </div>
      </aside>

      <div class='ml-content'>
      <!-- 首页：推荐流 -->
      <div v-if="musicStore.subTab === 'home'" class='ml-section-wrap'>
        <a-empty v-if='!musicStore.totalCount' description='暂无音乐，点击右上角“扫描”开始收录'>
          <template #image>
            <Music :size='56' :stroke-width='1.5' style='color: var(--color-text-3)' />
          </template>
        </a-empty>
        <template v-else>
          <div class='ml-section'>
            <div class='ml-section-head'>
              <div class='ml-section-title'>最近添加</div>
              <div class='ml-section-sub'>{{ musicStore.recentlyAdded.length }} 首</div>
            </div>
            <div class='ml-grid'>
              <div
                v-for='t in musicStore.recentlyAdded'
                :key='t.id'
                class='ml-card'
                @click='playFromList(musicStore.recentlyAdded, t)'
              >
                <div class='ml-card-cover'>
                  <img v-if='t.cover_url || t.thumbnail' :src='t.cover_url || t.thumbnail' alt='' />
                  <Music v-else :size='36' :stroke-width='1.5' />
                </div>
                <div class='ml-card-title' :title='t.title || t.file_name'>{{ t.title || t.file_name }}</div>
                <div class='ml-card-sub' :title='t.artist'>{{ t.artist || '未知艺人' }}</div>
              </div>
            </div>
          </div>

          <div class='ml-section'>
            <div class='ml-section-head'>
              <div class='ml-section-title'>随机推荐</div>
              <a-button type='text' size='mini' @click='musicStore.rerollRandom()'>
                <template #icon><RefreshCw :size='12' :stroke-width='1.8' /></template>
                换一批
              </a-button>
            </div>
            <div class='ml-grid'>
              <div
                v-for='t in musicStore.randomPicks'
                :key='t.id'
                class='ml-card'
                @click='playFromList(musicStore.randomPicks, t)'
              >
                <div class='ml-card-cover'>
                  <img v-if='t.cover_url || t.thumbnail' :src='t.cover_url || t.thumbnail' alt='' />
                  <Music v-else :size='36' :stroke-width='1.5' />
                </div>
                <div class='ml-card-title' :title='t.title || t.file_name'>{{ t.title || t.file_name }}</div>
                <div class='ml-card-sub' :title='t.artist'>{{ t.artist || '未知艺人' }}</div>
              </div>
            </div>
          </div>
        </template>
      </div>

      <!-- 全部 -->
      <div v-else-if="musicStore.subTab === 'all'" class='ml-list-wrap'>
        <div v-if='!filteredAll.length' class='ml-empty'>
          <a-empty description='暂无歌曲' />
        </div>
        <div v-else class='ml-tracklist'>
          <div
            v-for='(t, i) in filteredAll'
            :key='t.id'
            class='ml-track-row'
            @click='playFromList(filteredAll, t)'
          >
            <div class='ml-track-idx'>{{ i + 1 }}</div>
            <div class='ml-track-cover'>
              <img v-if='t.cover_url || t.thumbnail' :src='t.cover_url || t.thumbnail' alt='' />
              <Music v-else :size='18' :stroke-width='1.5' />
            </div>
            <div class='ml-track-main'>
              <div class='ml-track-title' :title='t.title || t.file_name'>{{ t.title || t.file_name }}</div>
              <div class='ml-track-meta' :title='t.artist'>{{ t.artist || '未知艺人' }}</div>
            </div>
            <div class='ml-track-album' :title='t.album'>{{ t.album || '' }}</div>
          </div>
        </div>
      </div>

      <!-- 艺人 -->
      <div v-else-if="musicStore.subTab === 'artists'" class='ml-grid-wrap'>
        <a-empty v-if='!musicStore.byArtist.length' description='暂无数据' />
        <div v-else class='ml-grid'>
          <div
            v-for='g in musicStore.byArtist'
            :key='g.artist'
            class='ml-card'
            @click='playFromList(g.items, g.items[0])'
          >
            <div class='ml-card-cover round'>
              <img v-if='artistCover(g)' :src='artistCover(g)' alt='' />
              <User v-else :size='36' :stroke-width='1.5' />
            </div>
            <div class='ml-card-title' :title='g.artist'>{{ g.artist }}</div>
            <div class='ml-card-sub'>{{ g.count }} 首</div>
          </div>
        </div>
      </div>

      <!-- 专辑 -->
      <div v-else-if="musicStore.subTab === 'albums'" class='ml-grid-wrap'>
        <a-empty v-if='!musicStore.byAlbum.length' description='暂无数据' />
        <div v-else class='ml-grid'>
          <div
            v-for='g in musicStore.byAlbum'
            :key='g.album'
            class='ml-card'
            @click='playFromList(g.items, g.items[0])'
          >
            <div class='ml-card-cover'>
              <img
                v-if='g.items[0]?.cover_url || g.items[0]?.thumbnail'
                :src='g.items[0].cover_url || g.items[0].thumbnail'
                alt=''
              />
              <Music v-else :size='36' :stroke-width='1.5' />
            </div>
            <div class='ml-card-title' :title='g.album'>{{ g.album }}</div>
            <div class='ml-card-sub'>{{ g.count }} 首</div>
          </div>
        </div>
      </div>

      <!-- 文件夹 -->
      <div v-else-if="musicStore.subTab === 'folders'" class='ml-grid-wrap'>
        <a-empty v-if='!musicStore.byFolder.length' description='暂无数据' />
        <div v-else class='ml-grid'>
          <div
            v-for='g in musicStore.byFolder'
            :key='g.path'
            class='ml-card'
            @click='playFromList(g.items, g.items[0])'
          >
            <div class='ml-card-cover folder'>
              <Folder :size='36' :stroke-width='1.5' />
            </div>
            <div class='ml-card-title' :title='g.path'>{{ g.name }}</div>
            <div class='ml-card-sub'>{{ g.count }} 首</div>
          </div>
        </div>
      </div>

      <!-- 收藏 -->
      <div v-else-if="musicStore.subTab === 'fav'" class='ml-list-wrap'>
        <div v-if='!musicStore.favoritesTracks.length' class='ml-empty'>
          <a-empty description='暂无收藏' />
        </div>
        <div v-else class='ml-tracklist'>
          <div
            v-for='(t, i) in musicStore.favoritesTracks'
            :key='t.id'
            class='ml-track-row'
            @click='playFromList(musicStore.favoritesTracks, t)'
          >
            <div class='ml-track-idx'>{{ i + 1 }}</div>
            <div class='ml-track-cover'>
              <img v-if='t.cover_url || t.thumbnail' :src='t.cover_url || t.thumbnail' alt='' />
              <Music v-else :size='18' :stroke-width='1.5' />
            </div>
            <div class='ml-track-main'>
              <div class='ml-track-title' :title='t.title || t.file_name'>{{ t.title || t.file_name }}</div>
              <div class='ml-track-meta' :title='t.artist'>{{ t.artist || '未知艺人' }}</div>
            </div>
            <div class='ml-track-album' :title='t.album'>{{ t.album || '' }}</div>
          </div>
        </div>
      </div>
    </div>
    </div>
  </div>
</template>

<style scoped>
.music-library {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 16px 20px 0 20px;
  overflow: hidden;
  background: var(--color-bg-1);
}

.ml-header {
  flex: 0 0 auto;
}

.ml-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ml-title {
  font-weight: 700;
  font-size: 22px;
  color: var(--color-text-1);
}

.ml-subtitle {
  margin-left: 12px;
  font-size: 12px;
  color: var(--color-text-3);
}

.flexauto {
  flex: 1 1 auto;
}

.ml-progress {
  margin-top: 10px;
  display: flex;
  align-items: center;
  gap: 12px;
}

.ml-progress-bar {
  position: relative;
  flex: 0 0 240px;
  height: 4px;
  background: var(--color-fill-2);
  border-radius: 4px;
  overflow: hidden;
}

.ml-progress-fill {
  position: absolute;
  inset: 0;
  background: linear-gradient(90deg, transparent, rgb(var(--primary-6)), transparent);
  background-size: 50% 100%;
  background-repeat: no-repeat;
  animation: ml-progress-slide 1.6s linear infinite;
}

@keyframes ml-progress-slide {
  0% { background-position: -50% 0; }
  100% { background-position: 150% 0; }
}

.ml-progress-text {
  font-size: 12px;
  color: var(--color-text-2);
}

.ml-body {
  display: grid;
  grid-template-columns: 192px minmax(0, 1fr);
  gap: 16px;
  flex: 1 1 auto;
  min-height: 0;
  margin-top: 16px;
}

.ml-side-nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px 8px 16px 0;
  border-right: 1px solid var(--color-border-2);
  overflow-y: auto;
}

.ml-nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  border-radius: 10px;
  font-size: 14px;
  color: var(--color-text-2);
  cursor: pointer;
  user-select: none;
  transition: background-color 0.15s, color 0.15s;
}

.ml-nav-item .ml-nav-icon {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
  opacity: 0.85;
}

.ml-nav-item:hover {
  background: var(--color-fill-2);
}

.ml-nav-item.active {
  color: rgb(var(--primary-6));
  background: rgba(var(--primary-6), 0.12);
  font-weight: 600;
}

.ml-nav-item.active .ml-nav-icon {
  opacity: 1;
}

.ml-nav-label {
  flex: 1 1 auto;
}

.ml-content {
  flex: 1 1 auto;
  overflow-y: auto;
  overflow-x: hidden;
  padding-bottom: 16px;
}

.ml-section {
  margin-bottom: 28px;
}

.ml-section-head {
  display: flex;
  align-items: center;
  margin-bottom: 12px;
  gap: 8px;
}

.ml-section-title {
  font-weight: 700;
  font-size: 16px;
  color: var(--color-text-1);
}

.ml-section-sub {
  font-size: 12px;
  color: var(--color-text-3);
}

.ml-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 16px;
}

.ml-card {
  cursor: pointer;
  user-select: none;
}

.ml-card-cover {
  position: relative;
  width: 100%;
  aspect-ratio: 1 / 1;
  background: var(--color-fill-2);
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-3);
  transition: transform 0.2s;
}

.ml-card-cover.round { border-radius: 50%; }
.ml-card-cover.folder { background: linear-gradient(135deg, #ffe7d6, #ffd6e0); color: #c45a3b; }
.ml-card-cover .iconfont { font-size: 36px; }

.ml-card-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.ml-card:hover .ml-card-cover {
  transform: translateY(-2px);
}

.ml-card-title {
  margin-top: 8px;
  font-size: 13px;
  color: var(--color-text-1);
  line-height: 1.3;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.ml-card-sub {
  margin-top: 2px;
  font-size: 12px;
  color: var(--color-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ml-tracklist {
  display: flex;
  flex-direction: column;
}

.ml-track-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background-color 0.15s;
}

.ml-track-row:hover { background: var(--color-fill-2); }

.ml-track-idx {
  width: 32px;
  text-align: center;
  font-size: 12px;
  color: var(--color-text-3);
}

.ml-track-cover {
  width: 40px;
  height: 40px;
  background: var(--color-fill-2);
  border-radius: 6px;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-text-3);
  flex: 0 0 auto;
}

.ml-track-cover img { width: 100%; height: 100%; object-fit: cover; }
.ml-track-cover .iconfont { font-size: 18px; }

.ml-track-main {
  flex: 1 1 auto;
  min-width: 0;
}

.ml-track-title {
  font-size: 14px;
  color: var(--color-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ml-track-meta {
  margin-top: 2px;
  font-size: 12px;
  color: var(--color-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ml-track-album {
  flex: 0 0 220px;
  font-size: 12px;
  color: var(--color-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ml-empty {
  padding: 60px 0;
  display: flex;
  justify-content: center;
}
</style>
