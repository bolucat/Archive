<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { Star, ListVideo, Play, RefreshCw, Shuffle, House, Folder } from 'lucide-vue-next'
import { t } from '../i18n'
import DB from '../utils/db'
import type { MediaLibraryItem } from '../types/media'
import type { UnifiedLibraryCard, UnifiedLibraryRow } from '../types/unifiedMediaLibrary'
import { LIBRARY_CATEGORY_IDS, MOVIE_CATEGORY_IDS, TV_CATEGORY_IDS, filterLibraryCategory, libraryCategoryGroups, libraryMovieSeries, libraryPlaylistMembers } from '../utils/unifiedLibraryCategories'
import { isMediaWatched } from '../utils/localWatchedState'
import { hasLocalMedia } from '../utils/unifiedMediaScope'
import { loadCustomSeries } from '../utils/customMediaSeries'
import { useMediaLibraryStore } from '../store/medialibrary'
import useBookmarks from '../store/unifiedLibraryBookmarks'
import useHomePreferences from '../store/unifiedHomePreferences'
import UnifiedMediaRow from './UnifiedMediaRow.vue'
import MediaLoadingIndicator from './MediaLoadingIndicator.vue'

const props = defineProps<{ route: string; mode: 'grid' | 'list'; localOnly: boolean; dailyItems: MediaLibraryItem[]; homeRoutes: string[]; card: (item: MediaLibraryItem) => UnifiedLibraryCard }>()
const emit = defineEmits<{ navigate: [route: string]; play: [items: MediaLibraryItem[], mode: 'play' | 'loop' | 'shuffle', title: string]; title: [title: string]; customSeries: [id: string]; toggleHome: [route: string] }>()
const media = useMediaLibraryStore()
const bookmarks = useBookmarks()
const home = useHomePreferences()
bookmarks.ensureLoaded()
const library = ref<MediaLibraryItem[]>([])
const loading = ref(false)
const error = ref('')
let generation = 0
async function reload() {
  const current = ++generation
  loading.value = true
  error.value = ''
  try { const data = await DB.getMediaLibrary(); if (current === generation) library.value = data.items }
  catch (reason) { if (current === generation) error.value = String(reason) }
  finally { if (current === generation) loading.value = false }
}
onMounted(reload)
watch(() => props.route, reload)
const items = computed(() => props.localOnly ? library.value.filter(hasLocalMedia) : library.value)
const parts = computed(() => props.route.split('/'))
const root = computed(() => parts.value[0])
const category = computed(() => parts.value[1] || '')
function label(route: string): string {
  if (home.titles['catalog:' + route]) return home.titles['catalog:' + route]
  const [root, sub, value] = route.split('/')
  if (value) return decodeURIComponent(value)
  if (root === 'playlist' && sub) return decodeURIComponent(sub)
  if (root === 'series' && sub) return libraryMovieSeries(items.value).find(group => group.id === sub)?.title || t('unified.series')
  if (sub === 'all') return root === 'movies' ? t('unified.allMovies') : t('unified.allTv')
  if (sub === 'recent') return t('media.recentlyAdded')
  if (sub === 'unwatched') return t('media.unwatched')
  if (sub === 'movies') return t('media.movie')
  if (sub) return t(('libraryBrowse.' + sub) as Parameters<typeof t>[0])
  const labels: Record<string, string> = { library: t('media.library'), playlist: t('media.playlist'), series: t('unified.series'), recent: t('media.recentlyAdded'), unwatched: t('media.unwatched'), daily: t('unified.dailyPicks'), movies: t('media.movie'), tv: t('media.tv'), other: t('unified.other'), music: t('nav.music'), books: t('nav.books'), favorites: t('unified.sources') }
  return labels[root] || root
}
const title = computed(() => label(props.route))
watch(title, value => emit('title', value), { immediate: true })
const filtered = computed(() => filterLibraryCategory(items.value, props.route, item => isMediaWatched(item, media.watchedItems)))
const playlists = computed(() => Object.entries(media.playlists).map(([name, ids]) => ({ name, members: libraryPlaylistMembers(items.value, ids) })))
const series = computed(() => libraryMovieSeries(items.value))
const selectedMembers = computed(() => {
  const name = decodeURIComponent(category.value)
  if (root.value === 'playlist') return playlists.value.find(group => group.name === name)?.members || []
  if (root.value === 'series') return series.value.find(group => group.id === name)?.members || []
  if (root.value === 'daily') return props.localOnly ? props.dailyItems.filter(hasLocalMedia) : props.dailyItems
  return filtered.value
})
const tiles = computed(() => {
  if (props.route === 'library') return LIBRARY_CATEGORY_IDS.map(id => ({ key: id, title: label(id) }))
  if (props.route === 'movies' || props.route === 'tv') return (root.value === 'movies' ? MOVIE_CATEGORY_IDS : TV_CATEGORY_IDS).map(id => ({ key: root.value + '/' + id, title: label(root.value + '/' + id) }))
  if (['genres', 'ratings', 'years', 'resolution', 'certification'].includes(category.value) && parts.value.length === 2) return libraryCategoryGroups(filtered.value, category.value).map(group => ({ key: props.route + '/' + encodeURIComponent(group.name), title: group.name }))
  return []
})
const tilePage = computed(() => ['library', 'movies', 'tv'].includes(props.route) || (['genres', 'ratings', 'years', 'resolution', 'certification'].includes(category.value) && parts.value.length === 2))
const facetPage = computed(() => ['genres', 'ratings', 'years', 'resolution', 'certification'].includes(category.value) && parts.value.length === 2)
const groupCards = computed<UnifiedLibraryCard[]>(() => {
  if (facetPage.value) return libraryCategoryGroups(filtered.value, category.value).map(group => ({ key: props.route + '/' + encodeURIComponent(group.name), title: group.name, image: group.members.find(item => item.backdropUrl)?.backdropUrl || group.members[0]?.posterUrl, contextMenu: event => openMenu(event, props.route + '/' + encodeURIComponent(group.name)), action: () => emit('navigate', props.route + '/' + encodeURIComponent(group.name)) }))
  if (props.route === 'playlist') return playlists.value.map(group => ({ key: 'playlist/' + encodeURIComponent(group.name), title: group.name, image: group.members.find(item => item.backdropUrl)?.backdropUrl || group.members[0]?.posterUrl, subtitle: t('customSeries.memberCount', { count: group.members.length }), contextMenu: event => openMenu(event, 'playlist/' + encodeURIComponent(group.name)), action: () => emit('navigate', 'playlist/' + encodeURIComponent(group.name)) }))
  if (props.route === 'series') return [
    ...series.value.map(group => ({ key: 'series/' + group.id, title: group.title, image: group.members[0]?.posterUrl, subtitle: t('customSeries.memberCount', { count: group.members.length }), contextMenu: (event: MouseEvent) => openMenu(event, 'series/' + group.id), action: () => emit('navigate', 'series/' + group.id) })),
    ...loadCustomSeries().map(group => {
      const members = libraryPlaylistMembers(items.value, group.members.filter(member => !member.serverId).map(member => member.id))
      return { key: 'custom-series/' + group.id, title: group.title, image: members.find(item => item.posterUrl)?.posterUrl, subtitle: t('customSeries.memberCount', { count: group.members.length }), action: () => emit('customSeries', group.id) }
    })
  ]
  return selectedMembers.value.map(props.card)
})
const row = computed<UnifiedLibraryRow>(() => ({ key: props.route, title: title.value, cards: groupCards.value, grouped: facetPage.value || props.route === 'playlist', landscape: facetPage.value || props.route === 'playlist', more: () => {} }))
const menu = ref<{ route: string; x: number; y: number }>()
function closeMenuOnEscape(event: KeyboardEvent) { if (event.key === 'Escape') menu.value = undefined }
onMounted(() => document.addEventListener('keydown', closeMenuOnEscape))
onUnmounted(() => { generation++; document.removeEventListener('keydown', closeMenuOnEscape) })
function openMenu(event: MouseEvent, route: string) { event.preventDefault(); menu.value = { route, x: Math.max(8, Math.min(event.clientX, window.innerWidth - 240)), y: Math.max(8, Math.min(event.clientY, window.innerHeight - 280)) } }
function play(mode: 'play' | 'loop' | 'shuffle') {
  const route = menu.value?.route || props.route
  const members = route === 'daily' ? props.dailyItems.filter(item => !props.localOnly || hasLocalMedia(item)) : route.startsWith('playlist/') ? playlists.value.find(group => group.name === decodeURIComponent(route.slice(9)))?.members || [] : route.startsWith('series/') ? series.value.find(group => group.id === route.slice(7))?.members || [] : filterLibraryCategory(items.value, route, item => isMediaWatched(item, media.watchedItems))
  emit('play', members, mode, label(route))
  menu.value = undefined
}
defineExpose({ openMenu, play })
</script>

<template>
  <div class="library-browser" data-testid="library-category-browser">
    <div v-if="loading" class="loading"><MediaLoadingIndicator /></div>
    <div v-else-if="error" role="alert">{{ error }} <button @click="reload">{{ t('common.retry') }}</button></div>
    <div v-else-if="tilePage && !facetPage && tiles.length" class="category-grid" :class="{ 'category-list': mode === 'list' }">
      <button v-for="tile in tiles" :key="tile.key" class="category-tile" :data-category="tile.key" @click="emit('navigate', tile.key)" @contextmenu="openMenu($event, tile.key)"><div class="category-art"><Star :size="60" :stroke-width="1.6" /></div><strong>{{ tile.title }}</strong></button>
    </div>
    <div v-else-if="route === 'playlist' && !groupCards.length" class="empty-state"><ListVideo :size="100" /><h2>{{ t('media.playlist') }}</h2><p>{{ t('libraryBrowse.playlistHelp') }}</p></div>
    <UnifiedMediaRow v-else-if="groupCards.length && (!tilePage || facetPage)" :row="row" :mode="facetPage || route === 'playlist' ? 'list' : mode" :show-more="false" />
    <div v-else class="empty-state"><Folder :size="72" /><h2>{{ title }}</h2><p>{{ category === 'certification' ? t('libraryBrowse.unknown') : t('libraryBrowse.empty') }}</p></div>
    <Teleport to="body"><div v-if="menu" class="category-menu-dismiss" @pointerdown="menu = undefined" @contextmenu.prevent="menu = undefined" @keydown.esc="menu = undefined"><div class="category-menu" role="menu" :style="{ left: menu.x + 'px', top: menu.y + 'px' }" @pointerdown.stop>
      <template v-if="!['music', 'books'].includes(menu.route)">
        <button role="menuitem" @click="play('play')"><Play :size="18" />{{ t('unified.play') }}</button>
        <button role="menuitem" @click="play('loop')"><RefreshCw :size="18" />{{ t('unified.loopPlay') }}</button>
        <button role="menuitem" @click="play('shuffle')"><Shuffle :size="18" />{{ t('unified.shufflePlay') }}</button>
      </template>
      <button role="menuitem" @click="bookmarks.toggle('favorites', menu.route); menu = undefined"><Star :size="18" />{{ bookmarks.favorites.includes(menu.route) ? t('unified.removeFavoriteFolder') : t('unified.addToFavorites') }}</button>
      <button role="menuitem" @click="emit('toggleHome', menu.route); menu = undefined"><House :size="18" />{{ homeRoutes.includes(menu.route) ? t('unified.removeHome') : t('libraryBrowse.addHome') }}</button>
    </div></div></Teleport>
  </div>
</template>

<style scoped>
.loading{display:grid;place-items:center;min-height:56vh}
.category-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:28px 22px}
.category-tile{padding:0;border:0;background:transparent;color:var(--color-text-1);text-align:left;cursor:pointer;font:inherit}
.category-art{aspect-ratio:2/3;border-radius:20px;border:1px solid var(--color-border-3);background:#242626;display:grid;place-items:center;color:#ff8000}
.category-tile strong{display:block;margin-top:10px;font-size:16px;font-weight:500}
.category-list{display:flex;flex-direction:column;gap:0}
.category-list .category-tile{display:flex;align-items:center;gap:22px;padding:16px 0;border-bottom:1px solid var(--color-border-2)}
.category-list .category-art{width:100px;flex-shrink:0}
.empty-state{min-height:56vh;display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;color:var(--color-text-3)}
.empty-state h2{color:var(--color-text-1);font-size:30px;margin:24px 0 12px}
.empty-state p{max-width:540px;line-height:1.6;font-size:17px}
.category-menu-dismiss{position:fixed;inset:0;z-index:1200}
.category-menu{position:fixed;width:240px;padding:10px;border-radius:20px;background:var(--color-bg-3);border:1px solid var(--color-border-3);box-shadow:0 12px 36px #0005}
.category-menu button{display:flex;align-items:center;gap:14px;width:100%;border:0;border-radius:8px;background:transparent;color:var(--color-text-1);font:inherit;padding:12px;cursor:pointer}
.category-menu button svg{color:#ff8000}
.category-menu button:hover{background:var(--color-fill-2)}
button:focus-visible{outline:2px solid #ff8000;outline-offset:3px}
</style>
