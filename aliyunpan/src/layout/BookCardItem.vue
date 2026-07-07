<script setup lang='ts'>
import type { IBookItem } from '../types/book'

const props = defineProps<{
  book: IBookItem
  cardScale?: number
  selected?: boolean
  brokenCover?: boolean
}>()

const emit = defineEmits<{
  select: [book: IBookItem, event: MouseEvent]
  open: [book: IBookItem]
  contextmenu: [book: IBookItem, event: MouseEvent]
  favorite: [book: IBookItem, event: MouseEvent]
  detail: [book: IBookItem, event: MouseEvent]
  shelf: [book: IBookItem, event: MouseEvent]
  delete: [book: IBookItem, event: MouseEvent]
  coverError: [bookId: string]
}>()

const scale = props.cardScale ?? 1
const broken = ref(false)
const hovered = ref(false)

const FORMAT_COLORS: Record<string, string> = {
  PDF: 'rgba(55,170,81,.7)', TXT: 'rgba(251,191,16,1)', EPUB: 'rgba(33,165,241,1)',
  MOBI: 'rgba(255,108,110,1)', AZW3: '#ff9900', AZW: '#ff9900',
  MD: '#5e7fff', FB2: '#0063b1', DOCX: '#6867d1',
  CBT: '#00b6c2', CBZ: '#00b6c2', CB7: '#00b6c2', CBR: '#00b6c2', HTML: '#e67e22',
}
function color(ext?: string) { return FORMAT_COLORS[(ext || '').toUpperCase()] || 'rgba(104,103,209,1)' }
function fmt(ext?: string) { return (ext || 'BOOK').toUpperCase() }
function title(b: IBookItem) { return b.title || b.file_name || 'Untitled' }
function showCover(b: IBookItem) { return !props.brokenCover && !broken.value && !!(b.cover_url || b.thumbnail) }
function handleCoverError() {
  broken.value = true
  emit('coverError', props.book.id)
}

// Reading progress (match koodo getPercentage)
function pctLabel(): string {
  const p = props.book.reading_progress
  if (typeof p !== 'number') return ''
  if (p <= 0) return 'New'
  if (p >= 100) return 'Done'
  return p.toFixed(2) + '%'
}
</script>

<template>
  <div
    class='book-card-item'
    :style='{ "--card-scale": scale }'
    @contextmenu.prevent='emit("contextmenu", book, $event)'
    @mouseenter='hovered = true'
    @mouseleave='hovered = false'
  >
    <!-- Cover (match koodo book-item-cover) -->
    <div
      class='book-card-item-cover'
      @click='emit("open", book)'
      @mousedown.prevent='(e: MouseEvent) => { if ((e as any).shiftKey) e.preventDefault() }'
      :style='{ boxShadow: showCover(book) ? "0px 4px 8px rgba(0,0,0,.12)" : "none", background: showCover(book) ? "transparent" : color(book.ext), alignItems: showCover(book) ? "center" : "flex-end" }'
    >
      <img v-if='showCover(book)' :src='book.cover_url || book.thumbnail' alt='' class='book-card-item-img' @error='handleCoverError' />
      <div v-else class='empty-cover-card' :style='{ transform: `scale(${scale})` }'>
        <div class='cover-banner' :style='{ backgroundColor: color(book.ext) }'>{{ fmt(book.ext) }}</div>
        <div class='cover-title'>{{ title(book) }}</div>
        <div class='cover-footer'>Koodo Reader</div>
      </div>
    </div>

    <!-- Select checkbox (match koodo book-selected-icon) -->
    <span v-if='hovered'
      class='book-card-select-icon'
      @click='(e: MouseEvent) => emit("select", book, e)'
      @mouseenter='hovered = true'>
      <CheckCircle v-if='selected' :size='18' />
      <Circle v-else :size='18' />
    </span>

    <!-- Title -->
    <div class='book-card-item-title'>{{ title(book) }}</div>

    <!-- Progress + Status bar -->
    <div style='display:flex;align-items:center;justify-content:space-between;width:80%;margin:2px auto 0'>
      <span v-if='pctLabel()' class='book-card-item-progress'>{{ pctLabel() }}</span>
      <span v-else style='width:1px' />
      <span style='display:flex;gap:4px;align-items:center'>
        <Heart v-if='book.is_favorite' :size='13' stroke-width='2' fill='rgb(231,69,69)' color='rgb(231,69,69)' />
        <MoreHorizontal :size='14' :stroke-width='2' style='opacity:.6;cursor:pointer' @click.stop='emit("contextmenu", book, $event)' />
      </span>
    </div>
    <div class='book-item-actions'>
      <button title='favorite' @click.stop='emit("favorite", book, $event)'><Heart :size='13' :stroke-width='2' /></button>
      <button title='detail' @click.stop='emit("detail", book, $event)'><MoreHorizontal :size='13' :stroke-width='2' /></button>
      <button title='shelf' @click.stop='emit("shelf", book, $event)'><LibraryBig :size='13' :stroke-width='2' /></button>
      <button title='delete' @click.stop='emit("delete", book, $event)'><Trash2 :size='13' :stroke-width='2' /></button>
    </div>
  </div>
</template>

<script lang='ts'>
import { ref } from 'vue'
import { Heart, MoreHorizontal, Circle, CheckCircle, LibraryBig, Trash2 } from 'lucide-vue-next'
</script>

<style scoped>
/* Match koodo bookCardItem.css exactly */
.book-card-item {
  width: calc(133px * var(--card-scale, 1));
  float: left;
  overflow: visible;
  position: relative;
}
.book-card-item-cover {
  width: calc(105px * var(--card-scale, 1));
  height: calc(137px * var(--card-scale, 1));
  margin: calc(10px * var(--card-scale, 1)) calc(15px * var(--card-scale, 1)) calc(5px * var(--card-scale, 1)) calc(15px * var(--card-scale, 1));
  cursor: pointer;
  display: flex;
  justify-content: center;
  border-radius: 2px;
  overflow: hidden;
}
.book-card-item-img {
  border-radius: 2px;
  object-fit: cover;
  width: 100%;
  height: 100%;
}
.book-card-item-title {
  width: 80%;
  margin-left: 10%;
  height: 31px;
  font-size: 12px;
  line-height: 15px;
  text-align: left;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
  position: relative;
  color: var(--color-text-1);
}
.book-card-item-progress {
  font-size: 12px;
  color: var(--color-text-3);
  display: inline-block;
}
.book-card-select-icon {
  position: absolute;
  bottom: 58px;
  right: 24px;
  cursor: pointer;
  color: var(--color-text-3);
  z-index: 2;
}
.book-item-actions {
  display:flex;
  justify-content:center;
  gap:4px;
  width:80%;
  margin:4px auto 0;
}
.book-item-actions button {
  width:22px;
  height:22px;
  border:0;
  border-radius:5px;
  background:transparent;
  color:var(--color-text-3);
  cursor:pointer;
  display:grid;
  place-items:center;
}
.book-item-actions button:hover {
  background:var(--color-fill-2);
  color:rgb(var(--primary-6));
}
/* Empty cover (match koodo emptyCover.css) */
.empty-cover-card {
  width: 105px;
  height: 137px;
  transform-origin: 0% 0%;
  position: relative;
}
.cover-banner {
  float: left;
  width: 90px;
  height: 30px;
  margin-top: 10px;
  line-height: 30px;
  font-weight: 500;
  text-indent: 10px;
  font-size: 15px;
  color: #fff;
}
.cover-title {
  float: left;
  font-size: 15px;
  font-weight: 500;
  opacity: 0.9;
  width: 90%;
  height: 54px;
  text-align: left;
  line-height: 18px;
  position: relative;
  top: 10px;
  margin-left: 5px;
  word-wrap: break-word;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
  overflow: hidden;
  color: var(--color-text-1);
}
.cover-footer {
  width: 100%;
  text-align: center;
  font-size: 13px;
  opacity: 0.3;
  position: absolute;
  bottom: 6px;
  color: var(--color-text-2);
}
</style>
