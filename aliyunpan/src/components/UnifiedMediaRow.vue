<script setup lang="ts">
import MediaPosterPlaceholder from './MediaPosterPlaceholder.vue'
import PosterRatingBadge from './PosterRatingBadge.vue'
import { Star } from 'lucide-vue-next'
import { ref, onMounted, onUnmounted } from 'vue'
import WatchedIndicator from './WatchedIndicator.vue'
import MediaPosterMenu from './MediaPosterMenu.vue'
import type { UnifiedLibraryCard } from '../types/unifiedMediaLibrary'
import type { UnifiedLibraryRow } from '../types/unifiedMediaLibrary'
import { t } from '../i18n'
import { detailBackdropUrl } from '../utils/mediaArtwork'

withDefaults(defineProps<{ row: UnifiedLibraryRow; mode?: 'horizontal' | 'grid' | 'list'; showMore?: boolean }>(), { mode: 'horizontal', showMore: true })
const menuCard = ref<UnifiedLibraryCard>()
const position = ref({ x: 0, y: 0 })
let clickTimer: ReturnType<typeof setTimeout> | undefined
function toggleWatched(card: UnifiedLibraryCard) { clearTimeout(clickTimer); card.posterMenu?.action('watched') }
function clickCard(card: UnifiedLibraryCard) { if (!card.posterMenu) { card.action(); return } clearTimeout(clickTimer); clickTimer = setTimeout(() => { clickTimer = undefined; card.action() }, 300) }
function openMenu(event: MouseEvent, card: UnifiedLibraryCard) { if (card.contextMenu) { card.contextMenu(event); return }; if (!card.posterMenu) return; event.preventDefault(); clearTimeout(clickTimer); menuCard.value = card; position.value = { x: Math.max(8, Math.min(event.clientX, window.innerWidth - 205)), y: Math.max(8, Math.min(event.clientY, window.innerHeight - 350)) } }
function closeMenu() { menuCard.value = undefined }
function keyMenu(event: KeyboardEvent) { if (event.key === 'Escape') closeMenu() }
onMounted(() => { document.addEventListener('click', closeMenu); document.addEventListener('keydown', keyMenu) })
onUnmounted(() => { clearTimeout(clickTimer); document.removeEventListener('click', closeMenu); document.removeEventListener('keydown', keyMenu) })
const failedImages = ref(new Set<string>())
</script>

<template>
  <section class="home-row" :class="`mode-${mode}`">
    <div v-if="showMore" class="row-heading">
      <h2>{{ row.title }}</h2>
      <button @click="row.more">{{ t('mediaServer.seeAllPlain') }}</button>
    </div>
    <div v-if="row.loading && !row.cards.length" role="status" class="shelf-status"><a-spin /> {{ t('common.loading') }}</div>
    <div v-else-if="row.error && !row.cards.length" role="status" class="shelf-status">{{ row.error }}</div>
    <div class="horizontal-row">
      <div v-for="card in row.cards" :key="card.key" class="media-card" role="button" tabindex="0" :class="{ landscape: row.landscape, grouped: row.grouped }" @keydown.enter.self.prevent="clickCard(card)" @keydown.space.self.prevent="clickCard(card)" @click="clickCard(card)" @dblclick.stop="openMenu($event, card)" @contextmenu="openMenu($event, card)">
        <div class="artwork">
          <MediaPosterPlaceholder :kind="row.key === 'resume' ? 'resume' : row.key === 'music' ? 'music' : row.key === 'books' ? 'book' : 'film'" />
          <img v-if="card.image && !failedImages.has(card.image)" :src="row.grouped ? detailBackdropUrl(card.image) : card.image" :alt="card.title" loading="lazy" @error="failedImages.add(card.image!)" />
          <WatchedIndicator v-if="row.key !== 'resume' && card.posterMenu" corner :watched="card.posterMenu.watched" />
          <PosterRatingBadge :rating="card.rating" />
          <span v-if="row.key === 'resume'" class="resume-play" aria-hidden="true">▶</span>
          <progress v-if="row.key === 'resume' || (card.progress || 0) > 0" :aria-label="card.title" :value="card.progress || 0" max="100" />
          <strong v-if="row.grouped" class="group-title">{{ card.title }}</strong>
        </div>
        <div v-if="mode === 'list' && !row.grouped && card.posterMenu" class="list-information">
          <strong :title="card.title">{{ card.title }}</strong>
          <div class="list-metadata">
            <span v-if="card.rating != null" class="list-rating"><Star :size="13" fill="currentColor" />{{ card.rating.toFixed(1) }}</span>
            <span v-if="card.subtitle">{{ card.subtitle }}</span>
            <span v-if="card.certification" class="classification">{{ card.certification }}</span>
          </div>
          <p class="list-overview">{{ card.overview || t('mediaServer.noOverview') }}</p>
          <div @click.stop @dblclick.stop @keydown.stop>
            <WatchedIndicator :watched="card.posterMenu.watched" @toggle="toggleWatched(card)" />
          </div>
        </div>
        <template v-else>
          <strong v-if="!row.grouped" :title="card.title">{{ card.title }}</strong>
          <small v-if="card.subtitle">{{ card.subtitle }}</small>
        </template>
      </div>
    </div>
    <Teleport to="body"><div v-if="menuCard?.posterMenu" class="home-poster-popup" :style="{ left: position.x + 'px', top: position.y + 'px' }" @click.stop><MediaPosterMenu hide-select :server="menuCard.posterMenu.server" :tv="menuCard.posterMenu.tv" :continuing="menuCard.posterMenu.continuing" :watched="menuCard.posterMenu.watched" :favorite="menuCard.posterMenu.favorite" :disabled="menuCard.posterMenu.disabled" @action="action => { menuCard?.posterMenu?.action(action); closeMenu() }" /></div></Teleport>
  </section>
</template>

<style scoped>
.home-poster-popup{position:fixed;z-index:1100;border:1px solid var(--color-border-3);border-radius:12px;box-shadow:0 8px 24px #0003;overflow:hidden}
.home-row { margin-bottom: 26px; }
.resume-play { position: absolute; inset: 0; margin: auto; width: 52px; height: 52px; border: 3px solid white; border-radius: 50%; display: grid; place-items: center; padding-left: 3px; box-sizing: border-box; color: white; font-size: 26px; background: #0002; filter: drop-shadow(0 2px 4px #0008); pointer-events: none; }
.row-heading { display: flex; justify-content: space-between; align-items: center; gap: 16px; margin-bottom: 14px; }
.shelf-status { min-height: 165px; display: flex; align-items: center; justify-content: center; gap: 12px; color: var(--color-text-3); }
h2 { font-size: 19px; font-weight: 600; margin: 0; }
button { font: inherit; color: inherit; cursor: pointer; }
button:focus-visible { outline: 2px solid #ff8b25; outline-offset: 2px; }
.row-heading button { flex-shrink: 0; border: 0; background: transparent; color: #ff8b25; font-size: 13px; }
.horizontal-row { display: flex; align-items: flex-start; gap: 24px; overflow-x: auto; padding: 3px 0 10px; scrollbar-width: none; }
.horizontal-row::-webkit-scrollbar { display: none; }
.media-card { width: 110px; flex-shrink: 0; text-align: left; border: 0; background: transparent; padding: 0; }
.media-card { cursor: pointer; color: inherit; font: inherit; }
.media-card:focus-visible { outline: 2px solid #ff8b25; outline-offset: 2px; }
.list-information { grid-row: 1 / 3; min-width: 0; display: flex; flex-direction: column; align-items: flex-start; gap: 6px; }
.list-information strong { margin: 0; font-size: 15px; max-width: 100%; }
.list-metadata { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; color: var(--color-text-3); font-size: 12px; }
.list-rating { display: inline-flex; align-items: center; gap: 5px; }
.classification { border: 1px solid currentColor; border-radius: 3px; padding: 0 3px; }
.list-overview { margin: 0; font-size: 13px; line-height: 1.4; color: var(--color-text-2); display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
.media-card.landscape { width: 280px; }
.artwork { height: 165px; position: relative; display: grid; place-items: center; border-radius: 16px; overflow: hidden; background: var(--color-fill-2); color: var(--color-text-4); }
.landscape .artwork { height: 158px; }
.artwork img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
.artwork progress { position: absolute; z-index: 3; left: 0; bottom: 0; width: 100%; height: 5px; margin: 0; border: 0; appearance: none; -webkit-appearance: none; background: #ffffff50; pointer-events: none; }
.artwork progress::-webkit-progress-bar { background: #ffffff50; }
.artwork progress::-webkit-progress-value { background: #ff8b25; }
.artwork progress::-moz-progress-bar { background: #ff8b25; }
.media-card strong, .media-card small { display: block; margin-top: 8px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; line-height: 16px; }
.media-card small { margin-top: 2px; color: var(--color-text-3); font-size: 11px; }
.mode-grid .horizontal-row { display: grid; grid-template-columns: repeat(auto-fill, 110px); gap: 24px; overflow: visible; }
.mode-grid .media-card { width: 100%; }
.mode-grid .artwork { height: auto; aspect-ratio: 2 / 3; }
.mode-grid .landscape .artwork { aspect-ratio: 16 / 9; }
.mode-list .horizontal-row { flex-direction: column; gap: 0; overflow: visible; }
.mode-list .media-card { width: 100%; display: grid; grid-template-columns: 160px minmax(0, 1fr); grid-template-rows: auto auto; align-items: center; gap: 8px 18px; padding: 16px 0; border-bottom: 1px solid var(--color-border-2); }
.mode-list .artwork { grid-row: 1 / 3; width: 110px; height: 165px; }
.mode-list .landscape .artwork { width: 160px; height: 90px; }
.mode-list strong { align-self: end; }
.mode-list .list-information strong { align-self: auto; }
.mode-list small { align-self: start; }
.mode-grid:has(.landscape) .horizontal-row { grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); }
.mode-list .media-card:not(.landscape) { grid-template-columns: 110px minmax(0, 1fr); }
.grouped .artwork::after { content: ''; position: absolute; inset: 0; background: linear-gradient(100deg, #0068ffb3, #8c68e6a6, #ff55c6b3); pointer-events: none; }
.grouped:nth-child(5n + 2) .artwork::after { background: linear-gradient(100deg, #ff8900b3, #ff2679a6, #7533ffb3); }
.grouped:nth-child(5n + 3) .artwork::after { background: linear-gradient(100deg, #087bd9a6, #29a2d99e); }
.grouped:nth-child(5n + 4) .artwork::after { background: linear-gradient(100deg, #a183ffa6, #f064c7a6, #ffae55b3); }
.grouped:nth-child(5n) .artwork::after { background: linear-gradient(100deg, #1bd9b6a6, #236affb3, #a251ffb3); }
.media-card .group-title { position: absolute; inset: 0; z-index: 2; display: flex; align-items: center; justify-content: center; margin: 0; padding: 20px; color: white; font-size: 22px; font-weight: 700; line-height: 1.3; text-align: center; white-space: normal; text-shadow: 0 2px 8px #0004; }
.mode-list .media-card.grouped { display: block; }
.mode-list .grouped .artwork { width: 100%; height: 244px; }
.mode-list .media-card.grouped { padding: 0; border: 0; }
.mode-list:has(.grouped) .horizontal-row { gap: 12px; }
@media (max-width: 700px) {
  .mode-list .grouped .artwork { height: 145px; }
  .media-card .group-title { font-size: 20px; }
}
</style>
