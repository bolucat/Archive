<script setup lang="ts">
/**
 * PodcastPanel — cloud-drive podcast management.
 * Auto-detects audio series from cloud drive folders, supports manual
 * marking, playback progress memory, and "continue listening".
 */
import { computed, ref } from 'vue'
import { Headphones, Link2, Play, Search, X } from 'lucide-vue-next'

export interface PodcastFolder {
  key: string
  name: string
  trackCount: number
  lastPlayedAt: number
}

export interface ExternalPodcastFeed {
  id: string
  title: string
  url: string
  addedAt: number
}

const props = defineProps<{
  folders: PodcastFolder[]
  externalFeeds?: ExternalPodcastFeed[]
  /** Map folder key → progress (0-1) */
  progressMap: Record<string, number>
  hiddenCount?: number
}>()

const emit = defineEmits<{
  (e: 'play', folder: PodcastFolder): void
  (e: 'continue', folder: PodcastFolder): void
  (e: 'toggle-mark', folder: PodcastFolder): void
  (e: 'restore-hidden'): void
  (e: 'add-external', feed: { title: string; url: string }): void
  (e: 'remove-external', feed: ExternalPodcastFeed): void
  (e: 'open-external', feed: ExternalPodcastFeed): void
}>()

const selected = ref<string | null>(null)
const query = ref('')
const externalTitle = ref('')
const externalUrl = ref('')

const sortedFolders = computed(() => {
  return [...props.folders].sort((a, b) => b.lastPlayedAt - a.lastPlayedAt)
})

const visibleFolders = computed(() => {
  const key = query.value.trim().toLowerCase()
  if (!key) return sortedFolders.value
  return sortedFolders.value.filter((folder) => folder.name.toLowerCase().includes(key))
})

const continueFolder = computed(() => sortedFolders.value.find((folder) => progress(folder) > 0))

function progress(folder: PodcastFolder): number {
  return props.progressMap[folder.key] || 0
}

function lastListened(folder: PodcastFolder): string {
  if (!folder.lastPlayedAt) return '未收听'
  const d = new Date(folder.lastPlayedAt)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

function addExternalFeed() {
  const url = externalUrl.value.trim()
  if (!url) return
  emit('add-external', { title: externalTitle.value.trim(), url })
  externalTitle.value = ''
  externalUrl.value = ''
}
</script>

<template>
  <div class="podcast-panel">
    <div class="podcast-head">
      <Headphones :size="15" :stroke-width="1.5" />
      <span class="podcast-title">播客</span>
      <span class="podcast-count" v-if="folders.length">{{ folders.length }} 个</span>
      <button v-if="props.hiddenCount" class="podcast-restore" title="恢复隐藏候选" @click="emit('restore-hidden')">恢复</button>
    </div>

    <button v-if="continueFolder" class="podcast-continue-card" type="button" @click="emit('continue', continueFolder)">
      <span>
        <small>继续收听</small>
        <b>{{ continueFolder.name }}</b>
      </span>
      <i>{{ Math.round(progress(continueFolder) * 100) }}%</i>
    </button>

    <label v-if="folders.length" class="podcast-search">
      <Search :size="13" :stroke-width="1.8" />
      <input v-model="query" placeholder="搜索播客文件夹" />
    </label>

    <div v-if="!folders.length" class="podcast-empty">
      暂无播客 — 在文件夹页右键标记为播客
    </div>

    <div v-else-if="!visibleFolders.length" class="podcast-empty">
      未找到匹配播客
    </div>

    <div v-for="f in visibleFolders" :key="f.key"
      :class="['podcast-row', selected === f.key ? 'selected' : '']"
      @click="selected = f.key"
    >
      <div class="podcast-meta">
        <div class="podcast-name">{{ f.name }}</div>
        <div class="podcast-info">
          {{ f.trackCount }} 集
          <span class="podcast-last">· {{ lastListened(f) }}</span>
          <span v-if="progress(f) > 0" class="podcast-progress">
            · {{ Math.round(progress(f) * 100) }}%
          </span>
        </div>
        <div v-if="progress(f) > 0" class="podcast-bar">
          <span :style="{ width: `${Math.round(progress(f) * 100)}%` }"></span>
        </div>
      </div>
      <div class="podcast-actions">
        <button v-if="progress(f) > 0" class="podcast-btn primary" @click.stop="emit('continue', f)" title="继续收听">
          <Play :size="12" :stroke-width="2" fill="currentColor" />
        </button>
        <button class="podcast-btn" @click.stop="emit('play', f)" title="播放最新">
          <Play :size="12" :stroke-width="2" />
        </button>
        <button class="podcast-rm" @click.stop="emit('toggle-mark', f)" title="取消标记">
          <X :size="12" />
        </button>
      </div>
    </div>

    <div class="podcast-external">
      <div class="podcast-external-title">
        <Link2 :size="13" :stroke-width="1.8" />
        <span>自定义播客链接</span>
      </div>
      <div class="podcast-external-form">
        <input v-model="externalTitle" placeholder="标题，可选" />
        <input v-model="externalUrl" placeholder="RSS 或播客主页链接" @keydown.enter="addExternalFeed" />
        <button type="button" @click="addExternalFeed">导入</button>
      </div>
      <div v-if="props.externalFeeds?.length" class="podcast-external-list">
        <div v-for="feed in props.externalFeeds" :key="feed.id" class="podcast-external-row">
          <button type="button" class="podcast-external-open" @click="emit('open-external', feed)">
            <b>{{ feed.title || feed.url }}</b>
            <small>{{ feed.url }}</small>
          </button>
          <button type="button" class="podcast-rm" title="删除链接" @click="emit('remove-external', feed)">
            <X :size="12" />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped lang="less">
.podcast-panel {
  padding: 12px; border-radius: 14px;
  border: 1px solid rgba(255,255,255,.07);
  background: rgba(255,255,255,.022);
  color: #fff;
}
.podcast-head { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; }
.podcast-title { font-size: 12.5px; font-weight: 700; color: rgba(255,255,255,.85); }
.podcast-count { font-size: 10.5px; color: rgba(255,255,255,.35); }
.podcast-restore {
  margin-left: auto; height: 22px; padding: 0 8px; border-radius: 999px;
  border: 1px solid rgba(0,245,212,.18); background: rgba(0,245,212,.07);
  color: rgba(210,255,250,.82); font-size: 10px; font-weight: 720; cursor: pointer;
}
.podcast-restore:hover { background: rgba(0,245,212,.12); color: #fff; }
.podcast-continue-card {
  width: 100%; min-height: 58px; display: flex; align-items: center; justify-content: space-between; gap: 12px;
  margin: 2px 0 10px; padding: 10px 12px; border: 1px solid rgba(244,210,138,.22); border-radius: 12px;
  background: linear-gradient(135deg, rgba(244,210,138,.12), rgba(0,245,212,.055));
  color: #fff; text-align: left; cursor: pointer;
}
.podcast-continue-card small { display: block; margin-bottom: 3px; color: rgba(244,210,138,.74); font-size: 10px; font-weight: 820; letter-spacing: .12em; }
.podcast-continue-card b { display: block; max-width: 170px; overflow: hidden; color: rgba(255,255,255,.92); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.podcast-continue-card i {
  flex: 0 0 auto; width: 42px; height: 42px; display: grid; place-items: center; border-radius: 50%;
  border: 1px solid rgba(0,245,212,.22); background: rgba(0,245,212,.08); color: rgba(210,255,250,.9);
  font-size: 11px; font-style: normal; font-weight: 850;
}
.podcast-search {
  height: 34px; display: flex; align-items: center; gap: 7px; margin: 0 0 9px; padding: 0 10px;
  border: 1px solid rgba(255,255,255,.08); border-radius: 11px; background: rgba(255,255,255,.032);
  color: rgba(255,255,255,.38);
}
.podcast-search input {
  min-width: 0; flex: 1; border: 0; outline: 0; background: transparent; color: rgba(255,255,255,.86);
  font-size: 11px; font-weight: 650;
}
.podcast-search input::placeholder { color: rgba(255,255,255,.3); }
.podcast-empty { padding: 20px; text-align: center; color: rgba(255,255,255,.3); font-size: 11px; }
.podcast-row {
  display: flex; align-items: center; gap: 10px; padding: 8px; border-radius: 10px;
  border: 1px solid rgba(255,255,255,.04); background: rgba(255,255,255,.018); cursor: pointer;
  margin-bottom: 6px; transition: all .15s;
}
.podcast-row:hover, .podcast-row.selected { background: rgba(0,245,212,.06); border-color: rgba(0,245,212,.16); }
.podcast-meta { flex: 1; min-width: 0; }
.podcast-name { font-size: 12px; font-weight: 650; color: rgba(255,255,255,.88); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.podcast-info { font-size: 10.5px; color: rgba(255,255,255,.38); margin-top: 2px; }
.podcast-progress { color: rgba(0,245,212,.7); }
.podcast-last { color: rgba(255,255,255,.30); }
.podcast-bar {
  height: 3px; margin-top: 6px; overflow: hidden; border-radius: 999px; background: rgba(255,255,255,.06);
}
.podcast-bar span {
  display: block; height: 100%; border-radius: inherit;
  background: linear-gradient(90deg, rgba(0,245,212,.78), rgba(244,210,138,.72));
}
.podcast-actions { display: flex; gap: 6px; flex-shrink: 0; }
.podcast-btn {
  width: 26px; height: 26px; border-radius: 7px; border: 1px solid rgba(255,255,255,.1);
  background: rgba(255,255,255,.04); color: rgba(255,255,255,.6); cursor: pointer;
  display: flex; align-items: center; justify-content: center;
}
.podcast-btn:hover { background: rgba(0,245,212,.1); border-color: rgba(0,245,212,.3); color: #fff; }
.podcast-btn.primary { background: rgba(0,245,212,.12); border-color: rgba(0,245,212,.32); color: #fff; }
.podcast-rm {
  width: 22px; height: 22px; border: none; border-radius: 5px;
  background: rgba(255,255,255,.04); color: rgba(255,255,255,.35); cursor: pointer;
  display: flex; align-items: center; justify-content: center; font-size: 12px;
}
.podcast-rm:hover { background: rgba(255,86,100,.14); color: rgba(255,122,144,.8); }
.podcast-external {
  margin-top: 10px; padding-top: 10px; border-top: 1px solid rgba(255,255,255,.06);
}
.podcast-external-title {
  display: flex; align-items: center; gap: 7px; margin-bottom: 8px;
  color: rgba(210,255,250,.72); font-size: 11px; font-weight: 760;
}
.podcast-external-form {
  display: grid; grid-template-columns: minmax(0, .75fr) minmax(0, 1.25fr) auto; gap: 6px;
}
.podcast-external-form input {
  min-width: 0; height: 30px; padding: 0 9px; border-radius: 9px;
  border: 1px solid rgba(255,255,255,.08); outline: 0; background: rgba(255,255,255,.035);
  color: rgba(255,255,255,.86); font-size: 10.5px; font-weight: 650;
}
.podcast-external-form input::placeholder { color: rgba(255,255,255,.28); }
.podcast-external-form button {
  height: 30px; padding: 0 10px; border-radius: 9px; border: 1px solid rgba(0,245,212,.18);
  background: rgba(0,245,212,.08); color: rgba(210,255,250,.9); font-size: 10.5px; font-weight: 780;
  cursor: pointer;
}
.podcast-external-form button:hover { background: rgba(0,245,212,.13); color: #fff; }
.podcast-external-list { margin-top: 8px; display: grid; gap: 6px; }
.podcast-external-row {
  display: flex; align-items: center; gap: 6px; padding: 6px; border-radius: 10px;
  border: 1px solid rgba(255,255,255,.045); background: rgba(255,255,255,.018);
}
.podcast-external-open {
  min-width: 0; flex: 1; border: 0; background: transparent; color: inherit; text-align: left; cursor: pointer;
}
.podcast-external-open b,
.podcast-external-open small {
  display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.podcast-external-open b { color: rgba(255,255,255,.86); font-size: 11px; font-weight: 720; }
.podcast-external-open small { margin-top: 2px; color: rgba(255,255,255,.34); font-size: 9.5px; }

@media (max-width: 760px) {
  .podcast-external-form { grid-template-columns: 1fr; }
}
</style>
