<script setup lang="ts">
import { Check, Clock3, Film, LoaderCircle, Plus, Tv } from 'lucide-vue-next'
import { tmdbImageUrl } from '../utils/tmdb'
import type { MediaAcquisitionRequest, MediaAcquisitionState } from '@shared/types/mediaAcquisition'
import { isMediaAcquisitionMovieUnreleased } from '@shared/mediaAcquisitionReleaseGate'

export interface TmdbMediaResult { id: number; media_type: 'movie' | 'tv'; title?: string; name?: string; original_title?: string; original_name?: string; overview?: string; poster_path?: string; release_date?: string; first_air_date?: string; vote_average?: number }

const props = withDefaults(defineProps<{ keyword: string; items: TmdbMediaResult[]; loading: boolean; error?: string; states?: MediaAcquisitionState[] }>(), { states: () => [] })
const emit = defineEmits<{ acquire: [request: MediaAcquisitionRequest] }>()

function titleOf(item: TmdbMediaResult) { return item.title || item.name || '未命名媒体' }
function yearOf(item: TmdbMediaResult) { return (item.release_date || item.first_air_date || '').slice(0, 4) }
function mediaKey(item: TmdbMediaResult) { return `${item.media_type}:tmdb:${item.id}` }
function stateOf(item: TmdbMediaResult) { return props.states.find(state => state.mediaKey === mediaKey(item)) }
function isTerminalFailure(state?: MediaAcquisitionState) { return state?.status === 'failed' || state?.status === 'cancelled' || state?.status === 'no_coverage' || state?.status === 'partial' }
function isCompleted(state?: MediaAcquisitionState) { return state?.status === 'completed' }
function isReserved(state?: MediaAcquisitionState) { return state?.status === 'reserved' }
function isWorking(state?: MediaAcquisitionState) { return !!state && !isTerminalFailure(state) && !isCompleted(state) && !isReserved(state) }
function isUnreleasedMovie(item: TmdbMediaResult) { return item.media_type === 'movie' && isMediaAcquisitionMovieUnreleased(item.release_date) }
function acquire(item: TmdbMediaResult) {
  const state = stateOf(item)
  if (isWorking(state) || isCompleted(state) || isReserved(state)) return
  emit('acquire', { tmdbId: item.id, mediaType: item.media_type, title: titleOf(item), alternativeTitles: [...new Set([item.original_title, item.original_name].filter((title): title is string => !!title && title !== titleOf(item)))], year: Number(yearOf(item)) || undefined, releaseDate: item.release_date })
}
function stageIndex(state: MediaAcquisitionState) {
  return ({ queued: 0, searching: 0, selecting: 1, transferring: 2, verifying: 2, organizing: 3, retry_wait: 2, completed: 4 } as Record<string, number>)[state.status] || 0
}
function stageText(state: MediaAcquisitionState) {
  if (state.status === 'queued') return state.activity || '等待搜索资源'
  return ({ queued: '等待搜索资源', searching: '正在搜索资源', selecting: '正在筛选候选资源', transferring: '正在转存到网盘', verifying: '正在核对入库目录', organizing: '正在整理并扫描媒体库', retry_wait: '等待重试' } as Record<string, string>)[state.status] || state.activity
}
</script>

<template>
  <section class="media-search-results" aria-label="媒体搜索结果">
    <header class="media-search-heading"><div><h2>媒体结果</h2><p>{{ loading ? '正在匹配 TMDB 媒体...' : `${items.length} 个候选 · 选择获取后由 Agent 继续处理资源检索和转存` }}</p></div></header>
    <div v-if="loading" class="media-search-grid"><div v-for="index in 6" :key="index" class="media-search-skeleton" /></div>
    <div v-else-if="items.length" class="media-search-grid">
      <article v-for="item in items" :key="`${item.media_type}-${item.id}`" class="media-search-card" :class="{ 'is-completed': isCompleted(stateOf(item)), 'is-working': isWorking(stateOf(item)) }">
        <div class="media-search-poster"><img v-if="item.poster_path" :src="tmdbImageUrl(item.poster_path, 'w342')" :alt="titleOf(item)" loading="lazy" /><component :is="item.media_type === 'tv' ? Tv : Film" v-else :size="30" /><span>{{ item.media_type === 'tv' ? '剧集' : '电影' }}</span></div>
        <div class="media-search-copy"><div class="media-search-title-row"><h3>{{ titleOf(item) }}</h3><button v-if="isCompleted(stateOf(item))" type="button" class="media-acquire-button is-completed" disabled><Check :size="15" />已获取</button><button v-else-if="isReserved(stateOf(item))" type="button" class="media-acquire-button is-working" disabled><Clock3 :size="14" />已预定</button><button v-else-if="isWorking(stateOf(item))" type="button" class="media-acquire-button is-working" disabled><LoaderCircle :size="14" class="spin" />获取中</button><button v-else type="button" class="media-acquire-button" @click="acquire(item)"><Plus :size="15" />{{ stateOf(item)?.status === 'failed' || stateOf(item)?.status === 'no_coverage' ? '重新获取' : isUnreleasedMovie(item) ? '预定' : '获取' }}</button></div><p class="media-search-meta">{{ yearOf(item) || '—' }}<template v-if="item.vote_average"> · {{ item.vote_average.toFixed(1) }} 分</template></p><p class="media-search-overview">{{ item.overview || '暂无剧情简介' }}</p><div v-if="isWorking(stateOf(item))" class="media-acquisition-state"><div class="media-acquisition-progress"><i :style="{ width: `${Math.max(5, stateOf(item)!.progress)}%` }" /></div><div class="media-acquisition-stages"><span v-for="(stage, index) in ['搜索资源', '筛选资源', '转存到网盘', '入库完成']" :key="stage" :class="{ done: index < stageIndex(stateOf(item)!), active: index === stageIndex(stateOf(item)!) }">{{ stage }}</span></div><p>{{ stageText(stateOf(item)!) }}</p></div><p v-else-if="isReserved(stateOf(item))" class="media-acquisition-reserved"><Clock3 :size="14" />上映后将自动开始获取</p><p v-else-if="isCompleted(stateOf(item))" class="media-acquisition-completed"><Check :size="14" />已加入媒体库</p><p v-else-if="stateOf(item)?.status === 'partial'" class="media-acquisition-failed">已部分入库，仍可继续补全</p><p v-else-if="stateOf(item)?.status === 'no_coverage'" class="media-acquisition-failed">暂未找到可导入资源，可重新发起任务</p><p v-else-if="stateOf(item)?.status === 'failed'" class="media-acquisition-failed">上次获取失败，可重新发起任务</p></div>
      </article>
    </div>
    <p v-else-if="error" class="media-search-error">{{ error }}</p>
    <p v-else class="media-search-empty">未匹配到影视元数据，仍可查看下方全网资源搜索结果。</p>
  </section>
</template>

<style scoped>
.media-search-results { margin: 28px 48px 22px; color: #e7eaf0; }.media-search-heading { display: flex; align-items: end; justify-content: space-between; margin-bottom: 12px; }.media-search-heading h2 { margin: 0; font-size: 18px; }.media-search-heading p { margin: 4px 0 0; color: #8e949f; font-size: 12px; }.media-search-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 12px; }.media-search-card { display: flex; min-width: 0; gap: 14px; padding: 14px; border: 1px solid rgba(255,255,255,.07); border-radius: 9px; background: rgba(255,255,255,.035); }.media-search-card.is-working { border-color: rgba(96, 221, 131, .26); }.media-search-card.is-completed { border-color: rgba(96, 221, 131, .34); background: rgba(54, 164, 91, .08); }.media-search-poster { position: relative; display: flex; flex: 0 0 78px; align-items: center; justify-content: center; width: 78px; height: 112px; overflow: hidden; border-radius: 6px; color: #9097a3; background: #25272a; }.media-search-poster img { width: 100%; height: 100%; object-fit: cover; }.media-search-poster span { position: absolute; left: 5px; bottom: 5px; padding: 1px 5px; border-radius: 3px; color: #fff; background: rgba(0,0,0,.68); font-size: 10px; }.media-search-copy { min-width: 0; flex: 1; }.media-search-title-row { display: flex; align-items: flex-start; gap: 8px; }.media-search-title-row h3 { flex: 1; min-width: 0; margin: 0; overflow: hidden; color: #f2f4f7; font-size: 14px; line-height: 22px; text-overflow: ellipsis; white-space: nowrap; }.media-acquire-button { display: inline-flex; flex: 0 0 auto; align-items: center; gap: 3px; padding: 7px 13px; border: 0; border-radius: 999px; color: #062a18; background: #24dc72; font-size: 12px; font-weight: 750; cursor: pointer; }.media-acquire-button:hover { background: #49e58a; }.media-acquire-button:disabled { cursor: default; }.media-acquire-button.is-working { color: #d5d9df; background: #434952; }.media-acquire-button.is-completed { color: #66df94; background: rgba(40, 197, 103, .13); }.media-search-meta { margin: 5px 0 0; color: #a8aeb8; font-size: 12px; }.media-search-overview { display: -webkit-box; margin: 8px 0 0; overflow: hidden; color: #858c97; font-size: 12px; line-height: 18px; -webkit-box-orient: vertical; -webkit-line-clamp: 3; }.media-acquisition-state { margin-top: 12px; }.media-acquisition-progress { height: 4px; overflow: hidden; border-radius: 99px; background: rgba(255,255,255,.12); }.media-acquisition-progress i { display: block; height: 100%; border-radius: inherit; background: #60dd83; transition: width .25s ease; }.media-acquisition-stages { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 4px; margin-top: 6px; color: #747b87; font-size: 10px; }.media-acquisition-stages span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.media-acquisition-stages .done { color: #5fcf82; }.media-acquisition-stages .active { color: #d7dae0; }.media-acquisition-state p, .media-acquisition-completed, .media-acquisition-failed { display: flex; align-items: center; gap: 4px; margin: 7px 0 0; color: #a8aeb8; font-size: 12px; }.media-acquisition-completed { color: #63d98a; }.media-acquisition-failed { color: #e7a17e; }.media-search-skeleton { height: 140px; border-radius: 9px; background: rgba(255,255,255,.05); animation: search-pulse 1.2s ease-in-out infinite; }.media-search-empty, .media-search-error { margin: 0; padding: 20px; color: #9197a1; border-radius: 8px; background: rgba(255,255,255,.03); font-size: 13px; text-align: center; }.media-search-error { color: #f4a4a4; }.spin { animation: search-spin .8s linear infinite; }
.media-acquisition-reserved { display: flex; align-items: center; gap: 4px; margin: 7px 0 0; color: #a9b6df; font-size: 12px; }
@keyframes search-pulse { 50% { opacity: .45; } } @keyframes search-spin { to { transform: rotate(360deg); } } @media(max-width:720px) { .media-search-results { margin: 20px 16px 18px; }.media-search-grid { grid-template-columns: 1fr; }.media-search-card { padding: 12px; } }
</style>
