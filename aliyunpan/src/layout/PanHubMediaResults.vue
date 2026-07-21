<script setup lang="ts">
import { Film, LoaderCircle, Plus, Tv } from 'lucide-vue-next'
import { tmdbImageUrl } from '../utils/tmdb'
import type { MediaAcquisitionRequest, MediaAcquisitionState } from '@shared/types/mediaAcquisition'
import { isMediaAcquisitionMovieUnreleased } from '@shared/mediaAcquisitionReleaseGate'

export interface TmdbMediaResult { id: number; media_type: 'movie' | 'tv'; title?: string; name?: string; original_title?: string; original_name?: string; overview?: string; poster_path?: string; release_date?: string; first_air_date?: string; vote_average?: number }

const props = defineProps<{ keyword: string; items: TmdbMediaResult[]; loading: boolean; error?: string; states?: MediaAcquisitionState[] }>()
const emit = defineEmits<{ acquire: [request: MediaAcquisitionRequest] }>()

function titleOf(item: TmdbMediaResult) { return item.title || item.name || '未命名媒体' }
function yearOf(item: TmdbMediaResult) { return (item.release_date || item.first_air_date || '').slice(0, 4) }
function isUnreleasedMovie(item: TmdbMediaResult) { return item.media_type === 'movie' && isMediaAcquisitionMovieUnreleased(item.release_date) }
function mediaKeyOf(item: TmdbMediaResult) { return `${item.media_type}:tmdb:${item.id}` }
function normalizeTitle(value: string) { return value.replace(/[\s\u3000]+/g, '').toLowerCase() }
function stateOf(item: TmdbMediaResult) {
  const exact = props.states?.find(state => state.mediaKey === mediaKeyOf(item))
  if (exact) return exact
  const title = normalizeTitle(titleOf(item))
  return props.states?.find(state => state.mediaType === item.media_type && normalizeTitle(state.title) === title)
}
function stateLabel(state?: MediaAcquisitionState) { return state?.status === 'completed' ? '已成功' : state?.status === 'failed' ? '失败' : state?.status === 'cancelled' ? '已取消' : state?.status === 'partial' ? '部分完成' : state?.status === 'no_coverage' ? '暂无资源' : state?.status === 'reserved' ? '已预定' : state ? state.activity : '' }
function stateClass(state?: MediaAcquisitionState) { return state?.status === 'completed' ? 'completed' : state && ['failed', 'cancelled'].includes(state.status) ? 'failed' : state && ['partial', 'no_coverage'].includes(state.status) ? 'warning' : 'working' }
function stateProgress(state?: MediaAcquisitionState) { return state?.status === 'completed' ? 100 : Math.max(0, Math.min(100, state?.progress || 0)) }
function isTerminalState(state?: MediaAcquisitionState) { return !!state && ['completed', 'failed', 'cancelled', 'partial', 'no_coverage'].includes(state.status) }
function isActiveState(state?: MediaAcquisitionState) { return !!state && state.status !== 'reserved' && !isTerminalState(state) }
function acquire(item: TmdbMediaResult) {
  emit('acquire', { tmdbId: item.id, mediaType: item.media_type, title: titleOf(item), alternativeTitles: [...new Set([item.original_title, item.original_name].filter((title): title is string => !!title && title !== titleOf(item)))], year: Number(yearOf(item)) || undefined, releaseDate: item.release_date })
}
</script>

<template>
  <section class="media-search-results" aria-label="媒体搜索结果">
    <header class="media-search-heading"><div><h2>媒体结果</h2><p>{{ loading ? '正在匹配 TMDB 媒体...' : `${items.length} 个候选 · 选择获取后由 Agent 继续处理资源检索和转存` }}</p></div></header>
    <div v-if="loading" class="media-search-grid"><div v-for="index in 6" :key="index" class="media-search-skeleton" /></div>
    <div v-else-if="items.length" class="media-search-grid">
      <article
        v-for="item in items"
        :key="`${item.media_type}-${item.id}`"
        :class="['media-search-card', { 'is-working': isActiveState(stateOf(item)), 'is-completed': stateOf(item)?.status === 'completed' }]"
      >
        <div class="media-search-poster">
          <img v-if="item.poster_path" :src="tmdbImageUrl(item.poster_path, 'w342')" :alt="titleOf(item)" loading="lazy" />
          <component :is="item.media_type === 'tv' ? Tv : Film" v-else :size="30" />
          <span>{{ item.media_type === 'tv' ? '剧集' : '电影' }}</span>
        </div>
        <div class="media-search-copy">
          <div class="media-search-title-row">
            <h3>{{ titleOf(item) }}</h3>
            <button v-if="!stateOf(item)" type="button" class="media-acquire-button" @click="acquire(item)"><Plus :size="15" />{{ isUnreleasedMovie(item) ? '预定' : '获取' }}</button>
            <button
              v-else
              type="button"
              :class="['media-acquisition-panel', stateClass(stateOf(item))]"
              :disabled="!isTerminalState(stateOf(item))"
              :title="isTerminalState(stateOf(item)) ? `${stateLabel(stateOf(item))}，点击可重新获取` : stateLabel(stateOf(item))"
              @click="acquire(item)"
            >
              <span
                class="media-acquisition-track"
                role="progressbar"
                aria-label="媒体获取进度"
                aria-valuemin="0"
                aria-valuemax="100"
                :aria-valuenow="stateProgress(stateOf(item))"
              >
                <i :style="{ width: `${Math.max(4, stateProgress(stateOf(item)))}%` }" />
              </span>
              <span class="media-acquisition-activity">
                <LoaderCircle :size="12" :class="{ spin: isActiveState(stateOf(item)) }" />
                <span>{{ stateLabel(stateOf(item)) }}</span>
              </span>
            </button>
          </div>
          <p class="media-search-meta">{{ yearOf(item) || '—' }} · {{ item.media_type === 'tv' ? '剧集' : '电影' }}<template v-if="item.vote_average"> · {{ item.vote_average.toFixed(1) }} 分</template></p>
          <p class="media-search-overview">{{ item.overview || '暂无剧情简介' }}</p>
        </div>
      </article>
    </div>
    <p v-else-if="error" class="media-search-error">{{ error }}</p>
    <p v-else class="media-search-empty">未匹配到影视元数据，仍可查看下方全网资源搜索结果。</p>
  </section>
</template>

<style scoped>
.media-search-results { margin: 28px 48px 22px; color: #e7eaf0; }
.media-search-heading { display: flex; align-items: end; justify-content: space-between; margin-bottom: 12px; }
.media-search-heading h2 { margin: 0; font-size: 18px; }
.media-search-heading p { margin: 4px 0 0; color: #8e949f; font-size: 12px; }
.media-search-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 12px; }
.media-search-card { display: grid; grid-template-columns: 78px minmax(0, 1fr); align-items: start; min-width: 0; gap: 14px; min-height: 140px; padding: 14px; border: 1px solid rgba(255,255,255,.07); border-radius: 9px; background: rgba(255,255,255,.035); }
.media-search-card.is-working { border-color: rgba(96,221,131,.22); }
.media-search-card.is-completed { border-color: rgba(96,221,131,.3); background: rgba(54,164,91,.07); }
.media-search-poster { position: relative; display: flex; align-items: center; justify-content: center; width: 78px; height: 112px; overflow: hidden; border-radius: 6px; color: #9097a3; background: #25272a; }
.media-search-poster img { width: 100%; height: 100%; object-fit: cover; }
.media-search-poster span { position: absolute; left: 7px; bottom: 7px; padding: 2px 6px; border-radius: 4px; color: #fff; background: rgba(0,0,0,.68); font-size: 10px; }
.media-search-copy { min-width: 0; }
.media-search-title-row { display: flex; align-items: flex-start; gap: 10px; }
.media-search-title-row h3 { flex: 1; min-width: 0; margin: 0; overflow: hidden; color: #f2f4f7; font-size: 14px; line-height: 22px; text-overflow: ellipsis; white-space: nowrap; }
.media-acquire-button { display: inline-flex; flex: 0 0 auto; align-items: center; gap: 3px; padding: 7px 13px; border: 0; border-radius: 999px; color: #062a18; background: #24dc72; font-size: 12px; font-weight: 750; cursor: pointer; }
.media-acquire-button:hover { background: #49e58a; }
.media-search-meta { margin: 5px 0 0; color: #a8aeb8; font-size: 12px; }
.media-search-overview { display: -webkit-box; margin: 8px 0 0; overflow: hidden; color: #858c97; font-size: 12px; line-height: 18px; -webkit-box-orient: vertical; -webkit-line-clamp: 3; }
.media-acquisition-panel { display: block; flex: 0 0 92px; width: 92px; min-width: 0; padding: 1px 0 0; border: 0; color: inherit; background: transparent; font: inherit; text-align: left; }
.media-acquisition-panel:not(:disabled) { cursor: pointer; }
.media-acquisition-panel:disabled { cursor: default; }
.media-acquisition-track { display: block; width: 100%; height: 4px; overflow: hidden; border-radius: 99px; background: rgba(255,255,255,.15); }
.media-acquisition-track i { display: block; height: 100%; border-radius: inherit; background: #60dd83; transition: width .25s ease; }
.media-acquisition-panel.failed .media-acquisition-track i { background: #f06c6c; }
.media-acquisition-panel.warning .media-acquisition-track i { background: #e6b45e; }
.media-acquisition-activity { display: flex; align-items: center; min-width: 0; gap: 4px; margin-top: 5px; color: #a8aeb8; font-size: 10px; line-height: 14px; }
.media-acquisition-activity svg { flex: 0 0 auto; }
.media-acquisition-activity span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.media-acquisition-panel.completed .media-acquisition-activity { color: #63d98a; }
.media-acquisition-panel.failed .media-acquisition-activity { color: #e18a84; }
.media-acquisition-panel.warning .media-acquisition-activity { color: #d9b66d; }
.media-search-skeleton { height: 140px; border-radius: 9px; background: rgba(255,255,255,.05); animation: search-pulse 1.2s ease-in-out infinite; }
.media-search-empty, .media-search-error { margin: 0; padding: 20px; color: #9197a1; border-radius: 8px; background: rgba(255,255,255,.03); font-size: 13px; text-align: center; }
.media-search-error { color: #f4a4a4; }
.spin { animation: search-spin .8s linear infinite; }
@keyframes search-pulse { 50% { opacity: .45; } }
@keyframes search-spin { to { transform: rotate(360deg); } }
@media(max-width: 720px) {
  .media-search-results { margin: 20px 16px 18px; }
  .media-search-card { padding: 12px; }
  .media-acquisition-panel { flex-basis: 78px; width: 78px; }
}
</style>
