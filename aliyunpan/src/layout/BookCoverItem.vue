<script setup lang='ts'>
import type { IBookItem } from '../types/book'
import { Heart, MoreHorizontal, Circle, CheckCircle, LibraryBig, Trash2 } from 'lucide-vue-next'

const props = defineProps<{ book: IBookItem; selected?: boolean; brokenCover?: boolean }>()
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
function bookTitle(b: IBookItem) { return b.title || b.file_name || 'Untitled' }
function showCover(b: IBookItem) { return !props.brokenCover && !broken.value && !!(b.cover_url || b.thumbnail) }
function handleCoverError() {
  broken.value = true
  emit('coverError', props.book.id)
}

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
    class='book-cover-item'
    @contextmenu.prevent='emit("contextmenu", book, $event)'
  >
    <!-- Header bar (match koodo book-cover-item-header) -->
    <div class='book-cover-item-header'>
      <span class='book-cover-item-progress'>{{ pctLabel() }}</span>
      <span style='float:right;display:flex;gap:4px;align-items:center;margin-right:6px;margin-top:4px'>
        <Heart v-if='book.is_favorite' :size='14' stroke-width='2' fill='rgb(231,69,69)' color='rgb(231,69,69)' style='opacity:.8' />
        <MoreHorizontal :size='14' :stroke-width='2' style='opacity:.8;cursor:pointer' @click.stop='emit("contextmenu", book, $event)' />
      </span>
    </div>

    <!-- Cover (match koodo book-cover-item-cover) -->
    <div
      class='book-cover-item-cover'
      @click='emit("open", book)'
      @mouseenter='hovered = true'
      @mouseleave='hovered = false'
      :style='{ background: showCover(book) ? "transparent" : color(book.ext), boxShadow: showCover(book) ? "0px 14px 24px rgba(0,0,0,.2)" : "none", alignItems: showCover(book) ? "center" : "flex-end" }'
    >
      <img v-if='showCover(book)' :src='book.cover_url || book.thumbnail' alt='' class='book-cover-item-img' @error='handleCoverError' />
      <div v-else class='empty-cover-scale' style='transform:scale(1.14)'>
        <div class='cover-banner' :style='{ backgroundColor: color(book.ext) }'>{{ fmt(book.ext) }}</div>
        <div class='cover-title'>{{ bookTitle(book) }}</div>
        <div class='cover-footer'>Koodo Reader</div>
      </div>
      <!-- Select icon overlay -->
      <span v-if='hovered'
        class='book-cover-select-icon'
        @click.stop='(e: MouseEvent) => emit("select", book, e)'
        @mouseenter='hovered = true'>
        <CheckCircle v-if='selected' :size='18' />
        <Circle v-else :size='18' />
      </span>
    </div>

    <!-- Title (match koodo book-cover-item-title) -->
    <p class='book-cover-item-title' @click='emit("open", book)'>{{ bookTitle(book) }}</p>

    <!-- Author (match koodo book-cover-item-author) -->
    <p class='book-cover-item-author'>Author: {{ book.author || 'Unknown author' }}</p>

    <!-- Publisher -->
    <p class='book-cover-item-author'>Publisher: {{ book.publisher || '-' }}</p>

    <!-- Description (match koodo book-cover-item-desc) -->
    <div class='book-cover-item-desc' @click='emit("open", book)'>
      <div class='book-cover-item-desc-detail'>
        {{ book.summary || book.description || 'Empty' }}
      </div>
    </div>
    <div class='book-item-actions'>
      <button title='favorite' @click.stop='emit("favorite", book, $event)'><Heart :size='14' :stroke-width='2' /></button>
      <button title='detail' @click.stop='emit("detail", book, $event)'><MoreHorizontal :size='14' :stroke-width='2' /></button>
      <button title='shelf' @click.stop='emit("shelf", book, $event)'><LibraryBig :size='14' :stroke-width='2' /></button>
      <button title='delete' @click.stop='emit("delete", book, $event)'><Trash2 :size='14' :stroke-width='2' /></button>
    </div>
  </div>
</template>

<script lang='ts'>
import { ref } from 'vue'
</script>

<style scoped>
/* Match koodo bookCoverItem.css exactly */
.book-cover-item {
  width: 100%;
  overflow: hidden;
  position: relative;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  padding: 14px;
  background: var(--color-bg-2);
  display: flex;
  flex-wrap: wrap;
}
.book-cover-item:hover { background: var(--color-fill-1); }
.book-cover-item-header {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}
.book-cover-item-progress {
  padding: 2px 8px;
  border-radius: 4px;
  background: rgba(var(--primary-6), .14);
  color: rgb(var(--primary-6));
  font-size: 12px;
  font-weight: 600;
}
.book-cover-item-cover {
  width: 120px;
  height: 170px;
  margin-right: 15px;
  cursor: pointer;
  float: left;
  display: flex;
  justify-content: center;
  align-items: center;
  border-radius: 3px;
  overflow: hidden;
  position: relative;
  flex-shrink: 0;
}
.book-cover-item-img {
  border-radius: 3px;
  object-fit: cover;
  width: 100%;
  height: 100%;
}
.book-cover-select-icon {
  position: absolute;
  top: 8px;
  right: 8px;
  cursor: pointer;
  color: rgba(255,255,255,.85);
  z-index: 2;
}
.book-cover-item-title {
  width: calc(100% - 135px);
  margin-top: 10px;
  height: 35px;
  font-size: 15px;
  line-height: 17px;
  text-align: left;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
  font-weight: 500;
  cursor: pointer;
  color: var(--color-text-1);
}
.book-cover-item-author {
  width: calc(100% - 135px);
  margin-top: 5px;
  height: 18px;
  font-size: 13px;
  line-height: 17px;
  opacity: 0.8;
  text-align: left;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 1;
  overflow: hidden;
  color: var(--color-text-2);
}
.book-cover-item-desc {
  width: calc(100% - 135px);
  margin-top: 5px;
  font-size: 13px;
  line-height: 17px;
  opacity: 0.8;
  text-align: left;
  height: 52px;
}
.book-cover-item-desc-detail {
  width: 100%;
  height: 100%;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
  overflow: hidden;
  color: var(--color-text-2);
}
.book-item-actions {
  width:calc(100% - 135px);
  margin-left:135px;
  margin-top:8px;
  display:flex;
  gap:6px;
}
.book-item-actions button {
  width:26px;
  height:26px;
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
/* Empty cover */
.empty-cover-scale {
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
