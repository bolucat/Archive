<script setup lang="ts">
import { ref, computed } from 'vue'
import { Film, Tv, Loader2, AlertCircle, Search, X, Star, Calendar, Clock, Info } from 'lucide-vue-next'
import { TMDB_BASE_URL_PROXY, tmdbImageUrl } from '../../utils/tmdb'

interface MediaItem { id: string; title: string; cover: string; desc: string; url: string }
interface Detail {
  title: string; poster: string; overview: string; rating: number; year: string
  runtime: string; genres: string[]; status: string; tagline: string
}

const props = defineProps<{
  state: 'loading' | 'done' | 'error'
  category?: string
  movies?: MediaItem[]
  tv?: MediaItem[]
  error?: string
}>()

const emit = defineEmits<{ (e: 'search', title: string): void; (e: 'search-resource', title: string): void; (e: 'retry'): void }>()
const imgFailed = ref<Set<string>>(new Set())
const detailLoading = ref(false)
const detailItem = ref<Detail | null>(null)
const detailError = ref('')

const hasMovies = computed(() => (props.movies?.length || 0) > 0)
const hasTv = computed(() => (props.tv?.length || 0) > 0)

const displayTitle = computed(() => {
  const c = props.category || ''
  if (c.includes('douban-top250')) return '豆瓣 Top250'
  if (c.includes('douban-movie')) return '豆瓣新片榜'
  if (c.includes('douban-weekly')) return '豆瓣口碑榜'
  if (c.includes('douban-us-box')) return '北美票房榜'
  if (c.startsWith('TMDB') || c.startsWith('搜索:')) return c
  return c || '影视'
})

function extractTerm(title: string): string {
  return title.replace(/^【[\d.]+】/, '').replace(/^📺\s*/, '').trim()
}

async function fetchDetail(item: MediaItem) {
  detailLoading.value = true; detailError.value = ''; detailItem.value = null
  try {
    const isTv = item.title.startsWith('📺')
    const type = isTv ? 'tv' : 'movie'
    const resp = await fetch(`${TMDB_BASE_URL_PROXY}/${type}/${item.id}?language=zh-CN`)
    const data = await resp.json()
    if (data?.title || data?.name) {
      detailItem.value = {
        title: data.title || data.name || item.title,
        poster: tmdbImageUrl(data.poster_path) || item.cover,
        overview: data.overview || '',
        rating: data.vote_average || 0,
        year: (data.release_date || data.first_air_date || '').slice(0, 4),
        runtime: data.runtime ? `${data.runtime}分钟` : data.episode_run_time?.[0] ? `${data.episode_run_time[0]}分钟/集` : '',
        genres: (data.genres || []).map((g: any) => g.name),
        status: data.status || '',
        tagline: data.tagline || '',
      }
    } else {
      detailError.value = '获取详情失败'
    }
  } catch (e: any) { detailError.value = e?.message || '获取详情失败' }
  finally { detailLoading.value = false }
}

function closeDetail() { detailItem.value = null; detailError.value = '' }
</script>

<template>
  <div class="ml-card">
    <div v-if="state === 'loading'" class="ml-status">
      <Loader2 :size="14" :stroke-width="2" class="ml-spin" />
      <span>正在获取{{ displayTitle }}...</span>
    </div>

    <template v-else-if="state === 'done'">
      <!-- Movies Section -->
      <div v-if="hasMovies" class="ml-section">
        <div class="ml-header">
          <Film :size="14" :stroke-width="1.5" class="ml-header-icon" />
          <span>电影 · {{ movies?.length || 0 }} 部</span>
        </div>
        <div class="ml-grid">
          <div v-for="m in movies" :key="m.id" class="ml-item" @click="fetchDetail(m)">
            <div class="ml-cover">
              <img v-if="m.cover && !imgFailed.has(m.id)" :src="m.cover" referrerpolicy="no-referrer" loading="lazy" class="ml-poster" @error="imgFailed.add(m.id)" />
              <Film v-else :size="28" :stroke-width="1.3" class="ml-poster-fb" />
              <div class="ml-search-overlay"><Info :size="14" :stroke-width="2" /></div>
            </div>
            <div class="ml-title">{{ extractTerm(m.title) }}</div>
            <div class="ml-desc">{{ m.desc }}</div>
          </div>
        </div>
      </div>

      <!-- TV Section -->
      <div v-if="hasTv" class="ml-section">
        <div class="ml-header">
          <Tv :size="14" :stroke-width="1.5" class="ml-header-icon" />
          <span>电视剧 · {{ tv?.length || 0 }} 部</span>
        </div>
        <div class="ml-grid">
          <div v-for="m in tv" :key="m.id" class="ml-item" @click="fetchDetail(m)">
            <div class="ml-cover">
              <img v-if="m.cover && !imgFailed.has(m.id)" :src="m.cover" referrerpolicy="no-referrer" loading="lazy" class="ml-poster" @error="imgFailed.add(m.id)" />
              <Tv v-else :size="28" :stroke-width="1.3" class="ml-poster-fb" />
              <div class="ml-search-overlay"><Info :size="14" :stroke-width="2" /></div>
            </div>
            <div class="ml-title">{{ extractTerm(m.title) }}</div>
            <div class="ml-desc">{{ m.desc }}</div>
          </div>
        </div>
      </div>

      <!-- Detail Card -->
      <div v-if="detailLoading" class="ml-status"><Loader2 :size="14" :stroke-width="2" class="ml-spin" /><span>加载详情...</span></div>
      <div v-else-if="detailError" class="ml-status ml-error"><AlertCircle :size="14" :stroke-width="1.5" />{{ detailError }}<button class="ml-retry" @click="closeDetail">关闭</button></div>
      <div v-else-if="detailItem" class="ml-detail">
        <button class="ml-detail-close" type="button" @click.stop="closeDetail"><X :size="16" :stroke-width="2" /></button>
        <div class="ml-detail-poster">
          <img v-if="detailItem.poster" :src="detailItem.poster" referrerpolicy="no-referrer" />
          <Film v-else :size="48" :stroke-width="1.3" class="ml-poster-fb" />
        </div>
        <div class="ml-detail-info">
          <h3 class="ml-detail-title">{{ detailItem.title }}</h3>
          <div v-if="detailItem.tagline" class="ml-detail-tagline">{{ detailItem.tagline }}</div>
          <div class="ml-detail-meta">
            <span v-if="detailItem.rating"><Star :size="13" :stroke-width="1.5" class="ml-star" />{{ detailItem.rating.toFixed(1) }}</span>
            <span v-if="detailItem.year"><Calendar :size="13" :stroke-width="1.5" />{{ detailItem.year }}</span>
            <span v-if="detailItem.runtime"><Clock :size="13" :stroke-width="1.5" />{{ detailItem.runtime }}</span>
          </div>
          <div v-if="detailItem.genres.length" class="ml-detail-genres">
            <span v-for="g in detailItem.genres" :key="g" class="ml-genre-tag">{{ g }}</span>
          </div>
        <p v-if="detailItem.overview" class="ml-detail-overview">{{ detailItem.overview }}</p>
        <button class="ml-search-res-btn" type="button" @click.stop="emit('search-resource', extractTerm(detailItem!.title))">🔍 搜索资源</button>
      </div>
      </div>

      <div v-if="!hasMovies && !hasTv" class="ml-empty">暂无数据</div>
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
.ml-section { border-bottom: 1px solid var(--color-border-2); }
.ml-section:last-child { border-bottom: 0; }
.ml-header { display: flex; align-items: center; gap: 6px; padding: 10px 14px; font-size: 12px; font-weight: 600; color: var(--color-text-3); }
.ml-header-icon { color: var(--color-text-3); }
.ml-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(110px, 1fr)); gap: 10px; padding: 10px 14px 14px; }
.ml-item { cursor: pointer; border-radius: 6px; transition: transform 0.1s; }
.ml-item:hover { transform: translateY(-2px); }
.ml-cover { position: relative; aspect-ratio: 2/3; border-radius: 6px; overflow: hidden; background: var(--color-fill-2); }
.ml-poster { width: 100%; height: 100%; object-fit: cover; display: block; }
.ml-poster-fb { position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); color: var(--color-text-4); }
.ml-search-overlay { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; background: rgba(0,0,0,0.5); color: #fff; opacity: 0; transition: opacity 0.15s; }
.ml-cover:hover .ml-search-overlay { opacity: 1; }
.ml-title { font-size: 12px; font-weight: 500; color: var(--color-text-2); margin-top: 5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ml-desc { font-size: 10px; color: var(--color-text-4); margin-top: 1px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ml-empty { padding: 16px 14px; font-size: 13px; color: var(--color-text-4); text-align: center; }

/* Detail */
.ml-detail { display: flex; gap: 14px; padding: 14px; position: relative; border-top: 1px solid var(--color-border-2); }
.ml-detail-close { position: absolute; top: 8px; right: 8px; display: flex; align-items: center; justify-content: center; width: 28px; height: 28px; padding: 0; color: var(--color-text-4); background: var(--color-fill-2); border: 0; border-radius: 6px; cursor: pointer; }
.ml-detail-close:hover { background: var(--color-fill-3); color: var(--color-text-2); }
.ml-detail-poster { flex-shrink: 0; width: 120px; border-radius: 6px; overflow: hidden; background: var(--color-fill-2); align-self: flex-start; }
.ml-detail-poster img { width: 100%; display: block; }
.ml-detail-info { min-width: 0; flex: 1; }
.ml-detail-title { margin: 0 0 4px; font-size: 17px; font-weight: 700; color: var(--color-text-1); line-height: 1.3; }
.ml-detail-tagline { font-size: 13px; color: var(--color-text-3); font-style: italic; margin-bottom: 8px; }
.ml-detail-meta { display: flex; flex-wrap: wrap; gap: 12px; margin-bottom: 6px; font-size: 13px; color: var(--color-text-3); }
.ml-detail-meta > span { display: flex; align-items: center; gap: 3px; }
.ml-star { color: #f59e0b; }
.ml-detail-genres { display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 8px; }
.ml-genre-tag { padding: 1px 8px; font-size: 11px; color: var(--color-text-2); background: var(--color-fill-2); border-radius: 10px; }
.ml-detail-overview { margin: 0 0 8px; font-size: 13px; line-height: 1.6; color: var(--color-text-2); }
.ml-search-res-btn { margin-top: 6px; padding: 5px 14px; font-size: 13px; color: #fff; background: rgb(var(--primary-6)); border: 0; border-radius: 6px; cursor: pointer; }
.ml-search-res-btn:hover { background: rgb(var(--primary-5)); }
.ml-retry { margin-left: auto; padding: 2px 10px; font-size: 12px; color: rgb(var(--primary-6)); background: transparent; border: 1px solid rgb(var(--primary-6)); border-radius: 4px; cursor: pointer; }
.ml-spin { animation: ml-spin 1s linear infinite; }
@keyframes ml-spin { to { transform: rotate(360deg); } }
</style>
