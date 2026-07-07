<script setup lang="ts">
import { ref } from 'vue'
import { Film, Loader2, AlertCircle, Search } from 'lucide-vue-next'

defineProps<{
  state: 'loading' | 'done' | 'error'
  category?: string
  movies?: { id: string; title: string; cover: string; desc: string; url: string }[]
  error?: string
}>()

const emit = defineEmits<{ (e: 'search', title: string): void; (e: 'retry'): void }>()
const imgFailed = ref<Set<string>>(new Set())

function extractTerm(title: string): string {
  return title.replace(/^【[\d.]+】/, '').trim()
}
</script>

<template>
  <div class="ml-card">
    <div v-if="state === 'loading'" class="ml-status">
      <Loader2 :size="14" :stroke-width="2" class="ml-spin" />
      <span>正在获取{{ category === 'douban-top250' ? '豆瓣 Top250' : category === 'douban-movie' ? '新片榜' : category === 'douban-weekly' ? '口碑榜' : category === 'douban-us-box' ? '北美票房' : '电影' }}...</span>
    </div>

    <template v-else-if="state === 'done' && movies">
      <div class="ml-header">
        <Film :size="14" :stroke-width="1.5" />
        <span>{{ category === 'douban-top250' ? '豆瓣 Top250' : category === 'douban-movie' ? '豆瓣新片榜' : category === 'douban-weekly' ? '豆瓣口碑榜' : category === 'douban-us-box' ? '北美票房榜' : '豆瓣影视' }}</span>
        <span class="ml-count">{{ movies.length }} 部</span>
      </div>
      <div v-if="movies.length" class="ml-grid">
        <div v-for="m in movies" :key="m.id" class="ml-item">
          <div class="ml-cover" @click="emit('search', extractTerm(m.title))">
            <img
              v-if="m.cover && !imgFailed.has(m.id)"
              :src="m.cover"
              referrerpolicy="no-referrer"
              loading="lazy"
              class="ml-poster"
              @error="imgFailed.add(m.id)"
            />
            <Film v-else :size="28" :stroke-width="1.3" class="ml-poster-fb" />
            <div class="ml-search-overlay">
              <Search :size="14" :stroke-width="2" />
            </div>
          </div>
          <div class="ml-title" @click="emit('search', extractTerm(m.title))">{{ extractTerm(m.title) }}</div>
          <div class="ml-desc">{{ m.desc }}</div>
        </div>
      </div>
      <div v-else class="ml-empty">暂无数据</div>
    </template>

    <div v-else-if="state === 'error'" class="ml-status ml-error">
      <AlertCircle :size="14" :stroke-width="1.5" />
      <span>{{ error }}</span>
      <button class="ml-retry" @click="emit('retry')">重试</button>
    </div>
  </div>
</template>

<style scoped>
.ml-card { margin: 8px 0; border-radius: 8px; background: var(--color-fill-1); border: 1px solid var(--color-border-2); overflow: hidden; }
.ml-status { display: flex; align-items: center; gap: 8px; padding: 12px 14px; font-size: 13px; color: var(--color-text-3); }
.ml-error { color: rgb(var(--danger-6)); }
.ml-header { display: flex; align-items: center; gap: 6px; padding: 10px 14px; font-size: 12px; font-weight: 600; color: var(--color-text-3); border-bottom: 1px solid var(--color-border-2); }
.ml-count { font-size: 11px; color: var(--color-text-4); font-weight: 400; }
.ml-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 12px; padding: 12px 14px; }
.ml-item { cursor: pointer; }
.ml-cover { position: relative; aspect-ratio: 2/3; border-radius: 6px; overflow: hidden; background: var(--color-fill-2); }
.ml-poster { width: 100%; height: 100%; object-fit: cover; display: block; }
.ml-poster-fb { position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); color: var(--color-text-4); }
.ml-search-overlay { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; background: rgba(0,0,0,0.5); color: #fff; opacity: 0; transition: opacity 0.15s; }
.ml-cover:hover .ml-search-overlay { opacity: 1; }
.ml-title { font-size: 12px; font-weight: 500; color: var(--color-text-2); margin-top: 6px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ml-title:hover { color: rgb(var(--primary-6)); }
.ml-desc { font-size: 10px; color: var(--color-text-4); margin-top: 2px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ml-empty { padding: 16px 14px; font-size: 13px; color: var(--color-text-4); text-align: center; }
.ml-retry { margin-left: auto; padding: 2px 10px; font-size: 12px; color: rgb(var(--primary-6)); background: transparent; border: 1px solid rgb(var(--primary-6)); border-radius: 4px; cursor: pointer; }
.ml-spin { animation: ml-spin 1s linear infinite; }
@keyframes ml-spin { to { transform: rotate(360deg); } }
</style>
