<script setup lang="ts">
import { computed, ref } from 'vue'
import { ListMusic, Plus, Trash2, Download, Upload, Edit3, X, Check } from 'lucide-vue-next'
import type { IPageMusicTrack } from '../../store/appstore'
import {
  type LocalPlaylist,
  loadPlaylists, savePlaylists,
  createPlaylist, removeTrackFromList, renamePlaylist,
  parseM3U, exportM3U,
} from '../../utils/mineradio/LocalPlaylistManager'

const props = defineProps<{ currentTracks: IPageMusicTrack[] }>()
const emit = defineEmits<{ (e: 'play', playlist: LocalPlaylist): void; (e: 'add-to-queue', tracks: IPageMusicTrack[]): void }>()

const playlists = ref<LocalPlaylist[]>(loadPlaylists())
const selected = ref<string | null>(null)
const editing = ref<string | null>(null)
const editName = ref('')
const showNew = ref(false)
const newName = ref('')
const selectedList = computed(() => playlists.value.find(p => p.id === selected.value) || null)

function persist() { savePlaylists(playlists.value) }

function onNew() {
  if (!newName.value.trim()) return
  playlists.value.unshift(createPlaylist(newName.value.trim(), props.currentTracks.slice(0, 10)))
  persist(); newName.value = ''; showNew.value = false; selected.value = playlists.value[0].id
}

function onDelete(id: string) { playlists.value = playlists.value.filter(p => p.id !== id); if (selected.value === id) selected.value = null; persist() }

function onRename(id: string) {
  const pl = playlists.value.find(p => p.id === id)
  if (!pl) return
  playlists.value = playlists.value.map(p => p.id === id ? renamePlaylist(pl, editName.value.trim() || pl.name) : p)
  persist(); editing.value = null
}

function onImportM3U() {
  const input = document.createElement('input'); input.type = 'file'; input.accept = '.m3u,.m3u8'
  input.onchange = async () => {
    const file = input.files?.[0]; if (!file) return
    const text = await file.text(); const paths = parseM3U(text); if (!paths.length) return
    const matched = props.currentTracks.filter(t => paths.some(p => p.endsWith(t.file_name) || t.file_name.endsWith(p.split('/').pop() || '')))
    const pl = createPlaylist(file.name.replace(/\.(m3u|m3u8)$/i, ''), matched)
    playlists.value.unshift(pl); persist(); selected.value = pl.id
  }
  input.click()
}

function onExport(pl: LocalPlaylist) {
  const blob = new Blob([exportM3U(pl)], { type: 'audio/x-mpegurl' })
  const a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = pl.name + '.m3u'; a.click(); URL.revokeObjectURL(a.href)
}

function onAddCurrentToSelected() {
  const pl = selectedList.value; if (!pl || !props.currentTracks.length) return
  const ids = new Set(pl.tracks.map(t => t.file_id))
  pl.tracks.push(...props.currentTracks.filter(t => !ids.has(t.file_id)))
  pl.updatedAt = Date.now(); persist()
}

function onRemoveTrack(pl: LocalPlaylist, fileId: string) {
  playlists.value = playlists.value.map(p => p.id === pl.id ? removeTrackFromList(pl, fileId) : p)
  persist()
}
</script>

<template>
  <div class="plm-panel">
    <div class="plm-head">
      <ListMusic :size="15" :stroke-width="1.5" />
      <span class="plm-title">本地歌单</span>
      <button class="plm-btn" @click="showNew = !showNew" title="新建歌单"><Plus :size="14" /></button>
      <button class="plm-btn" @click="onImportM3U" title="导入 M3U"><Upload :size="13" /></button>
    </div>

    <div v-if="showNew" class="plm-new-row">
      <input v-model="newName" class="plm-input" placeholder="歌单名称…" @keyup.enter="onNew" />
      <button class="plm-btn" @click="onNew"><Check :size="14" /></button>
      <button class="plm-btn" @click="showNew = false"><X :size="14" /></button>
    </div>

    <div class="plm-list">
      <div v-for="pl in playlists" :key="pl.id" :class="['plm-row', selected === pl.id ? 'selected' : '']" @click="selected = pl.id">
        <div v-if="editing === pl.id" class="plm-edit-row">
          <input v-model="editName" class="plm-input" :placeholder="pl.name" @keyup.enter="onRename(pl.id)" />
          <button class="plm-btn" @click.stop="onRename(pl.id)"><Check :size="12" /></button>
        </div>
        <template v-else>
          <div class="plm-meta"><div class="plm-name">{{ pl.name }}</div><div class="plm-count">{{ pl.tracks.length }} 首</div></div>
          <div class="plm-actions">
            <button class="plm-act" @click.stop="emit('play', pl)" title="播放">▶</button>
            <button class="plm-act" @click.stop="emit('add-to-queue', pl.tracks)" title="加入队列">+</button>
            <button class="plm-act" @click.stop="editing = pl.id; editName = pl.name" title="重命名"><Edit3 :size="11" /></button>
            <button class="plm-act" @click.stop="onExport(pl)" title="导出 M3U"><Download :size="11" /></button>
            <button class="plm-act danger" @click.stop="onDelete(pl.id)" title="删除"><Trash2 :size="11" /></button>
          </div>
        </template>
      </div>
      <div v-if="!playlists.length" class="plm-empty">暂无歌单 — 导入 M3U 或新建</div>
    </div>

    <div v-if="selectedList" class="plm-detail">
      <div class="plm-detail-head"><span>{{ selectedList.name }}</span><button class="plm-btn" @click="onAddCurrentToSelected">添加当前曲目</button></div>
      <div class="plm-track-list">
        <div v-for="t in selectedList.tracks" :key="t.file_id" class="plm-track">
          <span class="plm-track-name">{{ t.file_name }}</span>
          <button class="plm-act danger" @click="onRemoveTrack(selectedList, t.file_id)">×</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped lang="less">
.plm-panel { padding: 10px; border-radius: 14px; border: 1px solid rgba(255,255,255,.07); background: rgba(255,255,255,.022); color: #fff; }
.plm-head { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.plm-title { font-size: 12.5px; font-weight: 700; color: rgba(255,255,255,.85); flex: 1; }
.plm-btn { width: 24px; height: 24px; border-radius: 7px; border: 1px solid rgba(255,255,255,.1); background: rgba(255,255,255,.04); color: rgba(255,255,255,.55); cursor: pointer; display: flex; align-items: center; justify-content: center; }
.plm-btn:hover { background: rgba(0,245,212,.1); border-color: rgba(0,245,212,.3); color: #fff; }
.plm-new-row { display: flex; gap: 6px; margin-bottom: 8px; }
.plm-input { flex: 1; height: 26px; padding: 0 8px; border-radius: 7px; border: 1px solid rgba(255,255,255,.08); background: rgba(10,11,15,.8); color: rgba(255,255,255,.8); font-size: 11px; outline: none; }
.plm-input:focus { border-color: rgba(0,245,212,.3); }
.plm-list { max-height: 200px; overflow-y: auto; }
.plm-row { display: flex; align-items: center; justify-content: space-between; padding: 6px 8px; border-radius: 8px; border: 1px solid rgba(255,255,255,.03); background: rgba(255,255,255,.015); cursor: pointer; margin-bottom: 4px; transition: all .12s; }
.plm-row:hover, .plm-row.selected { background: rgba(0,245,212,.06); border-color: rgba(0,245,212,.14); }
.plm-meta { min-width: 0; }
.plm-name { font-size: 11.5px; font-weight: 650; color: rgba(255,255,255,.88); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.plm-count { font-size: 10px; color: rgba(255,255,255,.35); }
.plm-actions { display: flex; gap: 4px; flex-shrink: 0; }
.plm-act { width: 22px; height: 22px; border-radius: 5px; border: 1px solid rgba(255,255,255,.08); background: rgba(255,255,255,.03); color: rgba(255,255,255,.5); cursor: pointer; display: flex; align-items: center; justify-content: center; font-size: 11px; }
.plm-act:hover { background: rgba(0,245,212,.1); border-color: rgba(0,245,212,.25); color: #fff; }
.plm-act.danger:hover { background: rgba(255,86,100,.14); border-color: rgba(255,86,100,.3); color: #ff7a90; }
.plm-edit-row { display: flex; gap: 4px; flex: 1; }
.plm-empty { padding: 16px; text-align: center; color: rgba(255,255,255,.28); font-size: 11px; }
.plm-detail { margin-top: 10px; padding-top: 10px; border-top: 1px solid rgba(255,255,255,.06); }
.plm-detail-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px; font-size: 11.5px; font-weight: 700; }
.plm-detail-head button { width: auto; padding: 0 8px; font-size: 10px; }
.plm-track-list { max-height: 160px; overflow-y: auto; }
.plm-track { display: flex; align-items: center; justify-content: space-between; padding: 3px 6px; border-radius: 5px; margin-bottom: 2px; }
.plm-track:hover { background: rgba(255,255,255,.03); }
.plm-track-name { font-size: 10.5px; color: rgba(255,255,255,.6); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; flex: 1; }
</style>
