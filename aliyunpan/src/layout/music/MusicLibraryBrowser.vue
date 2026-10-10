<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { ArrowLeft, Disc3, Heart, Music, Play, Plus, Search, UserRound, ListMusic, Trash2, Pencil, Pause, SkipBack, SkipForward, X, Repeat, Repeat1, Shuffle, Volume2 } from 'lucide-vue-next'
import type { IMusicTrack } from '../../types/music'
import type { IPageMusicTrack } from '../../store/appstore'
import { createPlaylist, loadPlaylists, savePlaylists, addTracksToList, type LocalPlaylist } from '../../utils/radio/LocalPlaylistManager'
import { musicTrackKey, loadMusicTrackList, saveMusicTrackList } from '../../utils/musicPlayerStorage'
import useMusicPlayerStore from '../../store/musicplayerstore'
import message from '../../utils/message'

const props = defineProps<{ tracks: IMusicTrack[]; loading: boolean; hasMore: boolean; total: number }>()
const emit = defineEmits<{ (e: 'load-all'): void; (e: 'load-more'): void; (e: 'sources'): void; (e: 'play', tracks: IPageMusicTrack[], target?: IPageMusicTrack): void }>()
const player = useMusicPlayerStore()
type Tab = 'songs' | 'artists' | 'albums' | 'playlists'
const tab = ref<Tab>('songs')
const tabs: { key: Tab; title: string }[] = [{ key: 'songs', title: '歌曲' }, { key: 'artists', title: '歌手' }, { key: 'albums', title: '专辑' }, { key: 'playlists', title: '我的歌单' }]
const query = ref('')
const group = ref<{ title: string; ids: string[] } | null>(null)
const playlists = ref(loadPlaylists())
const selected = ref('favorites')
const favorites = ref(loadMusicTrackList('pm.favs', { legacyKeys: ['pageMusic.favorites'] }))
const form = ref<'create' | 'rename' | 'add' | 'assign' | 'delete' | null>(null)
const name = ref('')
const chosen = ref<string[]>([])
const assignTrack = ref<IPageMusicTrack | null>(null)
const addQuery = ref('')
const selectedList = computed(() => playlists.value.find(p => p.id === selected.value))
const title = (t: IMusicTrack) => t.title || t.file_name.replace(/\.[^.]+$/, '')
const cover = (t: IMusicTrack) => t.cover_url || t.thumbnail || ''
const failedCovers = ref(new Set<string>())
const favoriteKeys = computed(() => new Set(favorites.value.map(musicTrackKey)))
function playbackTrack(t: IMusicTrack): IPageMusicTrack {
  return { ...t, thumbnail: cover(t), password: '', icon: '' }
}
const metadata = computed(() => new Map(props.tracks.map(t => [musicTrackKey(t), t])))
function resolveTrack(t: IPageMusicTrack): IMusicTrack {
  return metadata.value.get(musicTrackKey(t)) || { ...t, id: musicTrackKey(t), ext: '', size: 0, category: 'audio', scanned_at: 0 }
}
const listTracks = computed(() => selected.value === 'favorites' ? favorites.value : selectedList.value?.tracks || [])
const displayed = computed(() => {
  let items = tab.value === 'playlists' ? listTracks.value.map(resolveTrack) : props.tracks
  if (group.value) items = items.filter(t => group.value!.ids.includes(t.id))
  const q = query.value.trim().toLocaleLowerCase()
  return q ? items.filter(t => `${title(t)} ${t.artist || ''} ${t.album || ''}`.toLocaleLowerCase().includes(q)) : items
})
const groups = computed(() => {
  const result = new Map<string, { title: string; artist: string; tracks: IMusicTrack[] }>()
  for (const track of displayed.value) {
    const artist = track.artist || '未知歌手'
    const label = tab.value === 'artists' ? artist : track.album || '未知专辑'
    const key = tab.value === 'artists' ? label : `${artist}\u0000${label}`
    const item = result.get(key) || { title: label, artist, tracks: [] }
    item.tracks.push(track)
    result.set(key, item)
  }
  return Array.from(result.entries())
})
const candidates = computed(() => props.tracks.filter(t => `${title(t)} ${t.artist || ''} ${t.album || ''}`.toLowerCase().includes(addQuery.value.trim().toLowerCase())))
function switchTab(value: Tab) { tab.value = value; group.value = null; if (value !== 'songs') emit('load-all') }
function openForm(value: typeof form.value) { if (form.value !== 'assign') assignTrack.value = null; form.value = value; name.value = value === 'rename' ? selectedList.value?.name || '' : ''; chosen.value = []; addQuery.value = ''; if (value === 'add') emit('load-all') }
function persist(next: LocalPlaylist[]) {
  try { savePlaylists(next); playlists.value = next; return true } catch { message.error('歌单保存失败，请重试'); return false }
}
function saveName() {
  if (!name.value.trim()) return
  if (form.value === 'create') {
    const item = createPlaylist(name.value.trim(), assignTrack.value ? [assignTrack.value] : [])
    if (!persist([item, ...playlists.value])) return
    switchTab('playlists'); selected.value = item.id
  } else if (!persist(playlists.value.map(p => p.id === selected.value ? { ...p, name: name.value.trim(), updatedAt: Date.now() } : p))) return
  form.value = null
}
function addTo(id: string, tracks: IPageMusicTrack[]) {
  if (persist(playlists.value.map(p => p.id === id ? addTracksToList(p, tracks) : p))) form.value = null
}
function removeTrack(track: IMusicTrack) {
  persist(playlists.value.map(p => p.id === selected.value ? { ...p, tracks: p.tracks.filter(t => musicTrackKey(t) !== musicTrackKey(track)), updatedAt: Date.now() } : p))
}
function deletePlaylist() {
  if (!persist(playlists.value.filter(p => p.id !== selected.value))) return
  selected.value = 'favorites'
  form.value = null
}
function toggleFavorite(track: IMusicTrack) {
  favorites.value = favoriteKeys.value.has(musicTrackKey(track)) ? favorites.value.filter(t => musicTrackKey(t) !== musicTrackKey(track)) : [...favorites.value, playbackTrack(track)]
  saveMusicTrackList('pm.favs', favorites.value)
  // Complete the legacy migration so removed favorites do not reappear.
  localStorage.removeItem('pageMusic.favorites')
}
function play(track = displayed.value[0]) {
  if (track) emit('play', displayed.value.map(t => tab.value === 'playlists' ? listTracks.value.find(p => musicTrackKey(p) === musicTrackKey(t)) || playbackTrack(t) : playbackTrack(t)), tab.value === 'playlists' ? listTracks.value.find(p => musicTrackKey(p) === musicTrackKey(track)) || playbackTrack(track) : playbackTrack(track))
}
function time(ms?: number) { if (!ms || !Number.isFinite(ms)) return '—'; const s = Math.floor(ms / 1000); return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}` }
function sync() { playlists.value = loadPlaylists(); favorites.value = loadMusicTrackList('pm.favs') }
onMounted(() => { window.addEventListener('storage', sync); window.addEventListener('focus', sync) })
onUnmounted(() => { window.removeEventListener('storage', sync); window.removeEventListener('focus', sync) })
</script>

<template>
  <section class="music-browser" aria-label="音乐资料库">
    <header class="music-browser-toolbar">
      <nav aria-label="音乐分类">
        <button v-for="item in tabs" :key="item.key" :class="{ active: tab === item.key }" :aria-current="tab === item.key ? 'page' : undefined" @click="switchTab(item.key)">{{ item.title }}</button>
      </nav>
      <label class="music-search"><Search :size="17" /><input v-model="query" placeholder="搜索歌曲、歌手、专辑" aria-label="搜索音乐" @input="emit('load-all')" /></label>
      <button class="primary" @click="openForm('create')"><Plus :size="17" />新建歌单</button>
    </header>
    <div class="music-browser-body">
      <aside v-if="tab === 'playlists'" class="music-playlists" aria-label="歌单">
        <button :class="{ selected: selected === 'favorites' }" @click="selected = 'favorites'"><Heart :size="24" /><span>我喜欢的<small>{{ favorites.length }} 首歌曲</small></span></button>
        <button v-for="item in playlists" :key="item.id" :class="{ selected: selected === item.id }" @click="selected = item.id"><ListMusic :size="24" /><span>{{ item.name }}<small>{{ item.tracks.length }} 首歌曲</small></span></button>
        <button @click="openForm('create')"><Plus :size="22" />新建歌单</button>
      </aside>
      <main class="music-results">
        <div v-if="group || tab === 'playlists'" class="music-collection-head">
          <button v-if="group" class="icon" aria-label="返回" @click="group = null"><ArrowLeft :size="22" /></button>
          <span v-if="tab === 'playlists'" class="playlist-art"><img v-if="displayed[0] && cover(displayed[0]) && !failedCovers.has(cover(displayed[0]))" :src="cover(displayed[0])" alt="" @error="failedCovers.add(cover(displayed[0]))" /><Heart v-else-if="selected === 'favorites'" :size="36" /><ListMusic v-else :size="36" /></span>
          <div><h2>{{ group?.title || selectedList?.name || '我喜欢的' }}</h2><p>{{ displayed.length }} 首歌曲</p></div>
          <template v-if="selectedList && tab === 'playlists'"><button @click="openForm('rename')"><Pencil :size="16" />编辑</button><button @click="openForm('add')"><Plus :size="16" />添加歌曲</button><button class="icon" aria-label="删除歌单" @click="openForm('delete')"><Trash2 :size="16" /></button></template>
        </div>
        <template v-if="(tab === 'artists' || tab === 'albums') && !group">
          <div class="music-collection-grid">
            <button v-for="[key, item] in groups" :key="key" class="music-collection" @click="group = { title: item.title, ids: item.tracks.map(t => t.id) }">
              <div class="music-art" :class="{ round: tab === 'artists' }"><img v-if="cover(item.tracks[0]) && !failedCovers.has(cover(item.tracks[0]))" :src="cover(item.tracks[0])" alt="" loading="lazy" @error="failedCovers.add(cover(item.tracks[0]))" /><UserRound v-else-if="tab === 'artists'" :size="48" /><Disc3 v-else :size="48" /></div>
              <strong>{{ item.title }}</strong><small>{{ tab === 'albums' ? item.artist : `${item.tracks.length} 首歌曲` }}</small>
            </button>
          </div>
        </template>
        <template v-else>
          <div class="music-list-actions"><span v-if="!group && tab === 'songs'">{{ total }} 首歌曲</span><button class="primary" :disabled="!displayed.length" @click="play()"><Play :size="16" />播放全部</button></div>
          <div class="music-table-head"><span>#</span><span>歌曲</span><span class="artist-column">歌手</span><span class="album-column">专辑</span><span>时长</span><span></span></div>
          <div v-for="(track, index) in displayed" :key="track.id" class="music-song">
            <span class="song-index">{{ index + 1 }}</span>
            <button class="song-name" @click="play(track)"><span class="song-cover"><img v-if="cover(track) && !failedCovers.has(cover(track))" :src="cover(track)" alt="" loading="lazy" @error="failedCovers.add(cover(track))" /><Music v-else :size="20" /></span><span>{{ title(track) }}<small class="compact-artist">{{ track.artist || '未知歌手' }}</small></span></button>
            <span class="artist-column">{{ track.artist || '未知歌手' }}</span><span class="album-column">{{ track.album || '—' }}</span><span class="song-duration">{{ time(track.duration_ms) }}</span>
            <div class="song-actions"><button class="icon" :class="{ liked: favoriteKeys.has(musicTrackKey(track)) }" :aria-label="favoriteKeys.has(musicTrackKey(track)) ? '取消喜欢' : '喜欢'" :aria-pressed="favoriteKeys.has(musicTrackKey(track))" @click="toggleFavorite(track)"><Heart :size="18" :fill="favoriteKeys.has(musicTrackKey(track)) ? 'currentColor' : 'none'" /></button><button v-if="selectedList && tab === 'playlists'" class="icon" aria-label="从歌单移除" @click="removeTrack(track)"><X :size="17" /></button><button v-else class="icon" aria-label="添加到歌单" @click="assignTrack = playbackTrack(track); form = 'assign'"><Plus :size="18" /></button></div>
          </div>
        </template>
        <p v-if="loading" role="status" class="music-empty">正在加载音乐…</p>
        <p v-else-if="!displayed.length" class="music-empty">{{ query ? '没有找到匹配的音乐' : tab === 'playlists' ? '歌单还是空的，添加喜欢的歌曲吧' : '暂无音乐，可从音乐来源导入' }}</p>
        <button v-if="hasMore && tab === 'songs' && !group" class="load-more" :disabled="loading" @click="emit('load-more')">加载更多</button>
      </main>
    </div>
    <footer v-if="player.state.hasTrack" class="music-now-playing">
      <button class="now-track" @click="player.togglePanel()"><img v-if="player.state.coverUrl" :src="player.state.coverUrl" alt="" /><Music v-else :size="24" /><span>{{ player.state.title }}<small>{{ player.state.artist }}</small></span></button>
      <div class="now-controls"><button class="icon" aria-label="上一首" @click="player.sendCommand('prev')"><SkipBack :size="20" /></button><button class="primary icon" :aria-label="player.state.isPlaying ? '暂停' : '播放'" @click="player.sendCommand('toggle')"><Pause v-if="player.state.isPlaying" :size="22" /><Play v-else :size="22" /></button><button class="icon" aria-label="下一首" @click="player.sendCommand('next')"><SkipForward :size="20" /></button></div>
      <button class="icon playback-mode" :title="{ list: '顺序播放', 'loop-list': '列表循环', 'loop-one': '单曲循环', shuffle: '随机播放' }[player.state.mode]" aria-label="切换播放模式" @click="player.sendCommand('mode')"><Shuffle v-if="player.state.mode === 'shuffle'" :size="18" /><Repeat1 v-else-if="player.state.mode === 'loop-one'" :size="18" /><Repeat v-else :size="18" /></button>
      <span>{{ time(player.state.currentTime * 1000) }}</span><input class="playback-seek" type="range" min="0" :max="player.state.duration || 1" :value="player.state.currentTime" :disabled="!player.state.duration" aria-label="播放进度" @change="player.sendCommand('seek', Number(($event.target as HTMLInputElement).value))" /><span>{{ time(player.state.duration * 1000) }}</span><label class="playback-volume"><Volume2 :size="18" /><input type="range" min="0" max="1" step="0.01" :value="player.state.volume" aria-label="音量" @input="player.sendCommand('volume', Number(($event.target as HTMLInputElement).value))" /></label><button class="icon" aria-label="播放队列" @click="player.sendCommand('queue')"><ListMusic :size="22" /></button>
    </footer>
    <footer v-else class="music-now-playing music-idle-player">
      <div class="now-track"><span class="song-cover"><Music :size="22" /></span><span>选择一首音乐<small>从你的音乐资料库开始播放</small></span></div>
      <div class="now-controls"><button class="icon" disabled aria-label="上一首"><SkipBack :size="20" /></button><button class="primary icon" :disabled="!displayed.length" aria-label="播放音乐" @click="play()"><Play :size="22" /></button><button class="icon" disabled aria-label="下一首"><SkipForward :size="20" /></button></div>
      <span class="idle-time">0:00</span><input class="playback-seek" type="range" min="0" max="1" value="0" disabled aria-label="尚未开始播放" /><span class="idle-time">0:00</span><Volume2 :size="18" />
    </footer>
    <a-modal :visible="form !== null" :title="form === 'create' ? '新建歌单' : form === 'rename' ? '编辑歌单' : form === 'delete' ? '删除歌单' : '添加歌曲'" :footer="false" @cancel="form = null">
      <form v-if="form === 'create' || form === 'rename'" class="music-form" @submit.prevent="saveName"><label>歌单名称<input v-model="name" autofocus maxlength="100" required /></label><button class="primary" :disabled="!name.trim()">保存</button></form>
      <div v-else-if="form === 'delete'" class="music-form"><p>删除“{{ selectedList?.name }}”？歌曲文件不会被删除。</p><button @click="deletePlaylist">删除歌单</button></div>
      <div v-else-if="form === 'assign'" class="music-form"><button v-for="item in playlists" :key="item.id" @click="assignTrack && addTo(item.id, [assignTrack])">{{ item.name }}</button><button @click="openForm('create')"><Plus :size="16" />新建歌单</button></div>
      <div v-else class="music-form"><input v-model="addQuery" placeholder="搜索要添加的歌曲" aria-label="搜索要添加的歌曲" /><div class="music-picker"><label v-for="track in candidates" :key="track.id"><input v-model="chosen" type="checkbox" :value="track.id" />{{ title(track) }} · {{ track.artist || '未知歌手' }}</label></div><button class="primary" :disabled="!chosen.length" @click="addTo(selected, tracks.filter(t => chosen.includes(t.id)).map(playbackTrack))">添加 {{ chosen.length }} 首歌曲</button></div>
    </a-modal>
  </section>
</template>

<style scoped>
.music-browser { height:100%; min-height:0; display:flex; flex-direction:column; color:var(--color-text-1); background:var(--color-bg-1); font-size:14px; }
.music-browser nav button { box-shadow:none; filter:none; background:transparent; }
.playlist-art { display:grid; place-items:center; width:88px; height:88px; flex-shrink:0; border-radius:12px; background:var(--color-fill-2); color:rgb(var(--primary-6)); overflow:hidden; }
.music-form { width:min(380px, calc(100vw - 80px)); }
button { color:inherit; font:inherit; border:0; background:transparent; cursor:pointer; display:inline-flex; align-items:center; justify-content:center; gap:8px; padding:9px 14px; border-radius:8px; }
button:hover { background:var(--color-fill-2); } button:focus-visible, input:focus-visible { outline:2px solid rgb(var(--primary-6)); outline-offset:2px; } button:disabled { opacity:.45; cursor:default; }
.primary { color:var(--color-white); background:rgb(var(--primary-6)); } .primary:hover { background:rgb(var(--primary-5)); }
.icon { width:36px; height:36px; padding:0; flex-shrink:0; }
.music-browser-toolbar { display:flex; align-items:center; gap:12px; padding:20px 28px 16px; flex-wrap:wrap; }
nav { display:flex; gap:16px; margin-right:auto; } nav button { border-radius:0; border-bottom:2px solid transparent; padding:12px 6px; white-space:nowrap; color:var(--color-text-2); } nav .active { color:rgb(var(--primary-6)); border-color:rgb(var(--primary-6)); font-weight:600; }
.music-search { display:flex; gap:8px; align-items:center; padding:8px 12px; border:1px solid var(--color-border-2); border-radius:20px; color:var(--color-text-3); } input { color:var(--color-text-1); background:transparent; border:0; outline:none; font:inherit; min-width:0; } .music-search input { width:190px; }
.music-browser-body { display:flex; flex:1; min-height:0; overflow:hidden; } .music-results { padding:0 28px 28px; flex:1; min-width:0; overflow:auto; }
.music-playlists { width:218px; flex-shrink:0; overflow:auto; padding:12px; border-right:1px solid var(--color-border-1); } .music-playlists button { display:flex; justify-content:flex-start; width:100%; gap:14px; text-align:left; margin-bottom:6px; } .music-playlists span { overflow:hidden; text-overflow:ellipsis; } .music-playlists .selected { color:rgb(var(--primary-6)); background:var(--color-fill-2); }
small { display:block; color:var(--color-text-3); font-size:12px; font-weight:400; margin-top:4px; }
.music-collection-head { display:flex; align-items:center; gap:12px; margin:20px 0; flex-wrap:wrap; } .music-collection-head h2 { font-size:24px; margin:0; } .music-collection-head p { color:var(--color-text-3); margin:6px 0; } .music-collection-head>div { margin-right:auto; }
.music-list-actions { display:flex; align-items:center; justify-content:flex-end; gap:16px; margin:8px 0 16px; color:var(--color-text-3); }
.music-song, .music-table-head { display:grid; grid-template-columns:32px minmax(150px,2.4fr) minmax(100px,1fr) minmax(100px,1fr) 52px 76px; align-items:center; gap:12px; border-bottom:1px solid var(--color-border-1); min-height:68px; } .music-table-head { min-height:34px; font-size:12px; color:var(--color-text-3); }
.music-song:hover { background:var(--color-fill-1); } .song-name { min-width:0; text-align:left; padding:8px 0; justify-content:flex-start; gap:14px; } .song-name>span:last-child { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; } .song-cover { width:44px; height:44px; display:grid; place-items:center; background:var(--color-fill-2); border-radius:6px; flex-shrink:0; } img { width:100%; height:100%; object-fit:cover; border-radius:inherit; } .artist-column, .album-column { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; color:var(--color-text-2); } .song-duration, .song-index { color:var(--color-text-3); font-variant-numeric:tabular-nums; } .song-actions { display:flex; } .liked { color:rgb(var(--primary-6)); } .compact-artist { display:none; }
.music-collection-grid { display:grid; grid-template-columns:repeat(auto-fill,minmax(150px,1fr)); gap:24px; padding-top:16px; } .music-collection { display:block; padding:0; text-align:left; min-width:0; } .music-art { width:100%; aspect-ratio:1; border-radius:8px; background:var(--color-fill-2); color:var(--color-text-3); display:grid; place-items:center; margin-bottom:12px; } .music-art.round { border-radius:50%; } .music-collection strong { display:block; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
.music-empty { text-align:center; color:var(--color-text-3); padding:56px 12px; } .load-more { display:flex; margin:20px auto; }
.music-now-playing { display:flex; align-items:center; gap:16px; padding:12px 24px; border-top:1px solid var(--color-border-1); } .now-track { width:240px; justify-content:flex-start; min-width:0; text-align:left; padding:0; } .now-track img { width:44px; height:44px; border-radius:6px; } .now-track span { overflow:hidden; white-space:nowrap; text-overflow:ellipsis; } .now-controls { display:flex; gap:10px; } progress { flex:1; min-width:40px; height:4px; accent-color:rgb(var(--primary-6)); }
.music-form { display:flex; flex-direction:column; gap:16px; color:var(--color-text-1); } .music-form input:not([type=checkbox]) { border:1px solid var(--color-border-2); border-radius:8px; padding:10px; display:block; width:100%; box-sizing:border-box; margin-top:8px; } .music-form button { border:1px solid var(--color-border-2); } .music-picker { max-height:320px; overflow:auto; } .music-picker label { display:flex; align-items:center; padding:10px 0; gap:10px; }
.playback-seek { flex:1; min-width:50px; accent-color:rgb(var(--primary-6)); } .playback-volume { display:flex; align-items:center; gap:8px; } .playback-volume input { width:70px; accent-color:rgb(var(--primary-6)); }
@media(max-width:1100px) { .playback-volume { display:none; } }
@media(max-width:700px) { .playback-seek,.playback-mode { display:none; } }
@media(max-width:1000px) { .artist-column { display:none; } .compact-artist { display:block; } .music-song,.music-table-head { grid-template-columns:24px minmax(130px,2fr) minmax(80px,1fr) 44px 72px; } .music-playlists { width:180px; } .now-track { width:160px; } }
@media(max-width:700px) { .music-browser-toolbar { padding:12px; gap:8px; } nav { width:100%; gap:20px; } .music-search { flex:1; } .music-search input { width:100%; } .music-results { padding:0 12px 20px; } .music-playlists { width:130px; padding:6px; } .music-playlists button { padding:10px 6px; } .music-playlists svg { display:none; } .album-column { display:none; } .music-song,.music-table-head { grid-template-columns:20px minmax(100px,1fr) 42px 72px; gap:6px; } .music-now-playing { padding:10px; gap:8px; } .music-now-playing>span,.music-now-playing progress { display:none; } .now-track { flex:1; } }
</style>

<style scoped>
/* Local music tokens keep both themes neutral and match the library's orange accent. */
.music-browser {
  --primary-6: 244, 102, 34;
  --primary-5: 230, 86, 20;
  --color-bg-1: #fafbfc;
  --color-text-1: #20242c;
  --color-text-2: #626978;
  --color-text-3: #858c98;
  --color-border-1: #e9ecf0;
  --color-border-2: #e1e5ea;
  --color-fill-1: #f0f2f5;
  --color-fill-2: #e9edf1;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
}
:global(body[arco-theme='dark'] .music-browser) {
  --color-bg-1: #181c1f;
  --color-text-1: #edf0f2;
  --color-text-2: #b0b7c2;
  --color-text-3: #858e99;
  --color-border-1: #292e33;
  --color-border-2: #363d44;
  --color-fill-1: #22282d;
  --color-fill-2: #2b3239;
}
.music-browser-toolbar { width:100%; max-width:1280px; box-sizing:border-box; margin:0 auto; padding:24px 36px 16px; gap:20px; }
nav { gap:28px; } nav button { font-size:15px; padding:10px 8px; }
.music-search { background:var(--color-fill-1); padding:9px 14px; }
.primary { border-radius:11px; padding:10px 18px; font-weight:500; box-shadow:none; }
.music-browser-body { width:100%; max-width:1280px; margin:0 auto; }
.music-results { padding:0 36px 24px; }
.music-list-actions { margin:0 0 18px; min-height:38px; font-size:12px; }
.music-song,.music-table-head { grid-template-columns:32px minmax(190px,2.1fr) minmax(120px,1fr) minmax(120px,1fr) 58px 86px; gap:14px; min-height:59px; }
.music-table-head { min-height:32px; }
.song-name { border-radius:0; padding:5px 0; gap:16px; font-weight:600; }
.song-name:hover { background:transparent; }
.song-cover { width:44px; height:44px; border-radius:6px; }
.song-index { padding-left:8px; font-size:13px; }
.music-song { transition:background .15s; }
.song-actions { gap:6px; }
.music-collection-grid { grid-template-columns:repeat(auto-fill,minmax(145px,1fr)); gap:22px; padding:12px 0 24px; }
.music-art { border-radius:7px; margin-bottom:10px; }
.music-collection:hover { background:transparent; }
.music-collection:hover .music-art { outline:2px solid rgb(var(--primary-6)); outline-offset:3px; }
.music-collection strong { font-size:15px; font-weight:600; }
.music-playlists { width:220px; padding:12px 16px; }
.music-playlists .selected { border-left:3px solid rgb(var(--primary-6)); border-radius:6px; }
.music-collection-head { margin:12px 0 20px; gap:18px; }
.playlist-art { width:112px; height:112px; border-radius:8px; }
.music-now-playing { min-height:70px; box-sizing:border-box; padding:12px max(28px,calc((100% - 1208px) / 2)); gap:20px; background:var(--color-bg-1); flex-shrink:0; }
.now-track { width:230px; gap:12px; font-size:13px; }
.now-controls { gap:14px; }
.now-controls .primary { border-radius:50%; width:40px; height:40px; padding:0; }
.music-now-playing>span { font-size:11px; color:var(--color-text-2); font-variant-numeric:tabular-nums; }
.music-idle-player .now-track { display:flex; align-items:center; }
.music-idle-player .playback-seek { opacity:.4; }
@media(max-width:1100px) {
  .music-browser-toolbar { padding:18px 24px; gap:12px; } nav { gap:18px; }
  .music-results { padding:0 24px 20px; }
  .music-song,.music-table-head { grid-template-columns:24px minmax(140px,2fr) minmax(90px,1fr) 48px 78px; }
  .artist-column { display:none; } .compact-artist { display:block; }
  .music-now-playing { padding:12px 24px; gap:12px; } .now-track { width:180px; }
}
@media(max-width:700px) {
  .music-browser-toolbar { padding:12px; } nav { gap:18px; }
  .music-results { padding:0 12px 18px; }
  .music-song,.music-table-head { grid-template-columns:20px minmax(100px,1fr) 42px 72px; gap:6px; }
  .music-playlists { width:130px; padding:6px; } .playlist-art { width:64px;height:64px; }
  .music-now-playing { padding:10px 12px; } .now-track { flex:1; }
}
</style>
