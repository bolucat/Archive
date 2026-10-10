<script setup lang="ts">
import { Play, Repeat, Shuffle, Heart, RefreshCw, Eye, ListPlus, Trash2, Star, Share, Download, SquareCheck, Pencil, Bookmark, Library } from 'lucide-vue-next'
import { mediaPosterActions, type PosterAction } from '../utils/mediaPosterMenu'
import { t } from '../i18n'
defineProps<{ server?: boolean; tv?: boolean; watched?: boolean; favorite?: boolean; continuing?: boolean; disabled?: PosterAction[]; hideSelect?: boolean }>()
defineEmits<{ action: [action: PosterAction] }>()
const icons = { play: Play, loop: Repeat, shuffle: Shuffle, favorite: Heart, refresh: RefreshCw, watched: Eye, playlist: ListPlus, delete: Trash2, transcode: Play, rating: Star, share: Share, download: Download, select: SquareCheck, metadata: Pencil, continue: Bookmark, series: Library }
const labels = { play: 'unified.play', loop: 'unified.loopPlay', shuffle: 'unified.shufflePlay', favorite: 'mediaServer.addFavorite', refresh: 'posterMenu.refresh', watched: 'mediaServer.markWatched', playlist: 'media.playlist', delete: 'common.delete', transcode: 'posterMenu.transcode', rating: 'posterMenu.rating', share: 'fileContext.share', download: 'fileContext.download', select: 'unified.selectItems', metadata: 'fileContext.editMetadata', continue: 'fileContext.continue', series: 'fileContext.series' } as const
</script>
<template><div class="media-poster-menu"><button v-for="action in mediaPosterActions(!!server, !!tv).filter(action => !hideSelect || action !== 'select')" :key="action" type="button" :disabled="disabled?.includes(action)" @click="$emit('action', action)"><component :is="icons[action]" :size="16" /><span>{{ t(action === 'continue' && continuing ? 'fileContext.removeContinue' : action === 'watched' && watched ? 'mediaServer.markUnwatched' : action === 'favorite' && favorite ? 'mediaServer.removeFavorite' : labels[action]) }}</span></button></div></template>
<style scoped>
.media-poster-menu{padding:5px;min-width:140px;border-radius:12px;background:var(--color-bg-popup);color:var(--color-text-1)}button{display:flex;align-items:center;gap:7px;width:100%;min-height:25px;padding:3px 9px;border:0;border-radius:5px;background:transparent;color:inherit;font-size:12px;text-align:left;white-space:nowrap;cursor:pointer}button svg{color:#ff8800;flex-shrink:0}button:hover:not(:disabled){background:rgb(var(--primary-6));color:white}button:disabled{opacity:.4;cursor:not-allowed}
</style>
