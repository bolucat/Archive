<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Bot, Film, RefreshCw } from 'lucide-vue-next'
import { tmdbImageUrl } from '../utils/tmdb'
import type { MediaAcquisitionRequest } from '@shared/types/mediaAcquisition'

type TrendingKind = 'movie' | 'tv' | 'anime' | 'douban-top250' | 'douban-movie' | 'douban-weekly' | 'douban-us-box'
type TrendingItem = { id: number | string; title?: string; name?: string; poster_path?: string; cover?: string; release_date?: string; first_air_date?: string; media_type?: 'movie' | 'tv' }

const props = defineProps<{ apiBase: string }>()
const emit = defineEmits<{ select: [title: string]; acquire: [request: MediaAcquisitionRequest] }>()

const recentTabs: { id: TrendingKind; label: string; path: string }[] = [
  { id: 'movie', label: '热门电影', path: 'hot-movies' },
  { id: 'tv', label: '热门剧集', path: 'hot-tv' },
  { id: 'anime', label: '热门动漫', path: 'hot-anime' },
]
const doubanTabs: { id: TrendingKind; label: string }[] = [
  { id: 'douban-top250', label: 'Top250' },
  { id: 'douban-movie', label: '新片榜' },
  { id: 'douban-weekly', label: '口碑榜' },
  { id: 'douban-us-box', label: '北美票房' },
]
const activeKind = ref<TrendingKind>('movie')
const items = ref<TrendingItem[]>([])
const loading = ref(false)
const loadingMore = ref(false)
const currentPage = ref(1)
const hasMore = ref(false)
const failedImages = ref<Set<string | number>>(new Set())
const activeTab = computed(() => recentTabs.find(tab => tab.id === activeKind.value))

function titleOf(item: TrendingItem) { return item.title || item.name || '未命名媒体' }
function yearOf(item: TrendingItem) { return (item.release_date || item.first_air_date || '').slice(0, 4) }

async function loadPage(page: number, append = false) {
  if (loading.value || loadingMore.value) return
  if (append) loadingMore.value = true
  else loading.value = true
  if (!append) failedImages.value = new Set()
  try {
    const response = activeTab.value
      ? await fetch(`${props.apiBase}/tmdb/${activeTab.value.path}?language=zh-CN&page=${page}`)
      : await fetch(`${props.apiBase}/douban-hot?category=${activeKind.value}&page=${page}&limit=25`)
    const payload = await response.json()
    if (!response.ok || payload?.code !== 0) throw new Error(payload?.message || '获取热门内容失败')
    const nextItems: TrendingItem[] = activeTab.value
      ? (Array.isArray(payload?.data?.results) ? payload.data.results : [])
      : (Array.isArray(payload?.data?.items) ? payload.data.items : [])
    items.value = append ? [...items.value, ...nextItems.filter(item => !items.value.some(existing => `${existing.media_type || activeKind.value}:${existing.id}` === `${item.media_type || activeKind.value}:${item.id}`))] : nextItems
    currentPage.value = page
    hasMore.value = activeTab.value ? page < Number(payload?.data?.total_pages || 0) : !!payload?.data?.hasMore
  } catch {
    if (!append) items.value = []
    hasMore.value = false
  } finally {
    loading.value = false
    loadingMore.value = false
  }
}

function load(force = false) {
  if (!force && items.value.length) return
  currentPage.value = 1
  hasMore.value = false
  return loadPage(1)
}

function loadMore() {
  if (!hasMore.value) return
  return loadPage(currentPage.value + 1, true)
}

function selectKind(kind: TrendingKind) {
  if (kind === activeKind.value) return
  activeKind.value = kind
  items.value = []
  currentPage.value = 1
  hasMore.value = false
  void load()
}

function imageOf(item: TrendingItem) {
  return item.poster_path ? tmdbImageUrl(item.poster_path, 'w342') : item.cover ? `${props.apiBase}/img?url=${encodeURIComponent(item.cover)}` : ''
}

function subtitleOf(item: TrendingItem) {
  if (activeKind.value.startsWith('douban-')) return '豆瓣影视'
  return `${yearOf(item) || '—'} · ${activeKind.value === 'anime' ? '动漫' : item.media_type === 'tv' || activeKind.value === 'tv' ? '剧集' : '电影'}`
}

function acquire(item: TrendingItem) {
  const mediaType = item.media_type || (activeKind.value === 'tv' ? 'tv' : activeKind.value === 'anime' ? 'anime' : 'movie')
  emit('acquire', {
    tmdbId: activeTab.value && typeof item.id === 'number' ? item.id : undefined,
    mediaType,
    title: titleOf(item),
    year: Number(yearOf(item)) || undefined,
    releaseDate: item.release_date || item.first_air_date
  })
}

watch(() => props.apiBase, () => { items.value = []; void load(true) })
onMounted(() => void load())
</script>

<template>
  <section class="media-trending" aria-label="近期热门">
    <header class="media-trending-head">
      <div class="media-trending-tabs">
        <h2>近期热门</h2>
        <button v-for="tab in recentTabs" :key="tab.id" type="button" :class="['media-trending-tab', { active: activeKind === tab.id }]" @click="selectKind(tab.id)">{{ tab.label }}</button>
        <span class="media-trending-divider">豆瓣影视</span>
        <button v-for="tab in doubanTabs" :key="tab.id" type="button" :class="['media-trending-tab', { active: activeKind === tab.id }]" @click="selectKind(tab.id)">{{ tab.label }}</button>
      </div>
      <button class="media-trending-refresh" type="button" title="刷新热门内容" :disabled="loading" @click="load(true)">
        <RefreshCw :size="14" :class="{ spin: loading }" />
      </button>
    </header>

    <div v-if="loading" class="media-trending-grid" aria-busy="true">
      <div v-for="index in 10" :key="index" class="media-trending-skeleton"><i /><b /></div>
    </div>
    <div v-else-if="items.length" class="media-trending-grid">
      <article v-for="(item, index) in items" :key="`${item.media_type || activeKind}-${item.id}`" class="media-trending-card" role="button" tabindex="0" @click="emit('select', titleOf(item))" @keydown.enter="emit('select', titleOf(item))" @keydown.space.prevent="emit('select', titleOf(item))">
        <span class="media-trending-poster">
          <img v-if="imageOf(item) && !failedImages.has(item.id)" :src="imageOf(item)" :alt="titleOf(item)" loading="lazy" referrerpolicy="no-referrer" @error="failedImages = new Set(failedImages).add(item.id)" />
          <Film v-else :size="26" :stroke-width="1.3" />
          <i class="media-trending-rank">#{{ index + 1 }}</i>
          <button class="media-trending-acquire" type="button" :aria-label="`AI 获取 ${titleOf(item)}`" @click.stop="acquire(item)"><Bot :size="13" />AI 获取</button>
        </span>
        <strong>{{ titleOf(item) }}</strong>
        <small>{{ subtitleOf(item) }}</small>
      </article>
    </div>
    <div v-else class="media-trending-empty">暂时无法获取热门内容</div>
    <button v-if="hasMore && !loadingMore" class="media-trending-more" type="button" @click="loadMore">加载更多...</button>
    <div v-else-if="loadingMore" class="media-trending-more-loading">加载更多...</div>
  </section>
</template>

<style scoped>
.media-trending { padding: 20px 24px 24px; color: #e8ebf2; background: #151617; border-radius: 10px; }
.media-trending-head { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 14px; }.media-trending-tabs { display: flex; flex: 1; flex-wrap: nowrap; align-items: center; gap: 8px; min-width: 0; overflow-x: auto; scrollbar-width: none; }.media-trending-tabs::-webkit-scrollbar { display: none; }.media-trending-tabs h2 { flex: 0 0 auto; margin: 0 8px 0 0; font-size: 21px; line-height: 30px; }.media-trending-tab { flex: 0 0 auto; border: 0; border-radius: 999px; padding: 6px 12px; color: #9da1ab; background: #242527; font-size: 13px; font-weight: 650; cursor: pointer; }.media-trending-tab.active { color: #171719; background: #f3f3f4; }.media-trending-divider { flex: 0 0 auto; padding-left: 9px; color: #70757e; font-size: 12px; font-weight: 650; }.media-trending-refresh { display: inline-flex; flex: 0 0 auto; align-items: center; justify-content: center; width: 30px; height: 30px; padding: 0; border: 0; border-radius: 50%; color: #b9bec8; background: transparent; cursor: pointer; }.media-trending-refresh:hover { color: #fff; background: #282a2d; }.media-trending-refresh:disabled { cursor: wait; }.spin { animation: trending-spin .8s linear infinite; }
.media-trending-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 14px; }.media-trending-card { min-width: 0; padding: 0; border: 0; color: inherit; background: transparent; text-align: left; cursor: pointer; outline: none; }.media-trending-card:focus-visible .media-trending-poster { outline: 2px solid #6b7df6; outline-offset: 3px; }.media-trending-poster { position: relative; display: flex; align-items: center; justify-content: center; width: 100%; aspect-ratio: 2 / 3; overflow: hidden; border-radius: 9px; color: #808691; background: #292b2e; box-shadow: 0 6px 14px rgba(0,0,0,.25); }.media-trending-poster img { width: 100%; height: 100%; object-fit: cover; }.media-trending-rank { position: absolute; left: 6px; top: 6px; padding: 2px 6px; border-radius: 999px; color: #fff; background: rgba(0,0,0,.68); font-size: 11px; font-style: normal; font-weight: 750; }.media-trending-acquire { position: absolute; right: 6px; bottom: 6px; display: inline-flex; align-items: center; gap: 3px; padding: 5px 7px; border: 0; border-radius: 5px; color: #071a13; background: #6de8b0; box-shadow: 0 2px 8px rgba(0,0,0,.28); font-size: 11px; font-weight: 750; cursor: pointer; }.media-trending-acquire:hover { background: #91f2c4; }.media-trending-card strong, .media-trending-card small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.media-trending-card strong { margin-top: 7px; color: #eef0f3; font-size: 13px; line-height: 18px; }.media-trending-card small { margin-top: 1px; color: #8e939d; font-size: 11px; line-height: 16px; }.media-trending-card:hover .media-trending-poster { transform: translateY(-2px); box-shadow: 0 10px 20px rgba(0,0,0,.34); }.media-trending-poster { transition: transform .16s, box-shadow .16s; }.media-trending-skeleton i { display: block; width: 100%; aspect-ratio: 2/3; border-radius: 9px; background: #292b2e; animation: trending-pulse 1.2s ease-in-out infinite; }.media-trending-skeleton b { display: block; width: 70%; height: 12px; margin-top: 8px; border-radius: 4px; background: #292b2e; }.media-trending-empty { padding: 32px 0; color: #8e939d; font-size: 13px; text-align: center; }
.media-trending-more, .media-trending-more-loading { display: block; margin: 18px auto 0; padding: 6px 14px; border: 0; border-radius: 999px; color: #aeb4bf; background: #242527; font-size: 12px; font-weight: 650; }.media-trending-more { cursor: pointer; }.media-trending-more:hover { color: #fff; background: #303236; }.media-trending-more-loading { background: transparent; color: #7d838e; }
@keyframes trending-spin { to { transform: rotate(360deg); } } @keyframes trending-pulse { 50% { opacity: .45; } }
@media (max-width: 720px) { .media-trending { padding: 16px; }.media-trending-grid { grid-template-columns: repeat(auto-fill, minmax(94px, 1fr)); gap: 10px; }.media-trending-tabs h2 { width: 100%; margin-bottom: 2px; } }
</style>
