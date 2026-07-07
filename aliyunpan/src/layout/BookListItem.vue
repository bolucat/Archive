<script setup lang='ts'>
import type { IBookItem } from '../types/book'
import { Heart, Circle, CheckCircle, MoreHorizontal, LibraryBig, Trash2 } from 'lucide-vue-next'

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
    class='book-list-item'
    @contextmenu.prevent='emit("contextmenu", book, $event)'
    @mouseenter='hovered = true'
    @mouseleave='hovered = false'
  >
    <!-- Cover (match koodo book-item-list-cover) -->
    <div
      class='book-list-item-cover'
      @click='emit("open", book)'
      :style='{ background: showCover(book) ? "transparent" : color(book.ext) }'
    >
      <img v-if='showCover(book)' :src='book.cover_url || book.thumbnail' alt='' class='book-list-item-img' style='width:100%' @error='handleCoverError' />
      <div v-else class='empty-cover-list' style='transform:scale(0.43)'>
        <div class='cover-banner' :style='{ backgroundColor: color(book.ext) }'>{{ fmt(book.ext) }}</div>
        <div class='cover-title'>{{ bookTitle(book) }}</div>
        <div class='cover-footer'>Koodo Reader</div>
      </div>
    </div>

    <!-- Select icon -->
    <span
      v-if='hovered'
      class='book-list-select-icon'
      @click='(e: MouseEvent) => emit("select", book, e)'
      @mouseenter='hovered = true'>
      <CheckCircle v-if='selected' :size='18' />
      <Circle v-else :size='18' />
    </span>

    <!-- Title / Subtitle / Author / Percentage (match koodo book-item-list-title) -->
    <div class='book-list-item-title' @click='emit("open", book)'>
      <div class='book-list-item-subtitle'>
        <div class='book-list-item-subtitle-text'>
          <!-- Status badges -->
          <span v-if='book.is_favorite' style='display:inline-flex;align-items:center;gap:4px;margin-right:4px;vertical-align:middle'>
            <Heart :size='14' stroke-width='2' fill='rgb(231,69,69)' color='rgb(231,69,69)' style='opacity:.8' />
          </span>
          {{ bookTitle(book) }}
        </div>
      </div>
      <!-- Percentage -->
      <p class='book-list-item-percentage'>{{ pctLabel() }}</p>
      <!-- Author -->
      <div class='book-list-item-author'>{{ book.author || 'Unknown author' }}</div>
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
/* Match koodo bookListItem.css exactly */
.book-list-item {
  margin: 0;
  padding: 12px;
  margin-right: 0;
  position: relative;
  display: flex;
  align-items: flex-start;
  border-bottom: 1px solid var(--color-border);
  min-height: 74px;
}
.book-list-item:hover { background: var(--color-fill-1); }
.book-list-item-cover {
  width: 43px;
  height: 65px;
  opacity: 1;
  cursor: pointer;
  border-radius: 1px;
  flex-shrink: 0;
  overflow: hidden;
  display: flex;
  justify-content: center;
}
.book-list-item-img {
  border-radius: 1px;
  object-fit: cover;
}
.book-list-select-icon {
  position: absolute;
  left: 30px;
  bottom: 10px;
  cursor: pointer;
  color: var(--color-text-3);
  z-index: 2;
}
.book-list-item-title {
  flex: 1;
  min-width: 0;
  height: 100%;
  cursor: pointer;
  display: flex;
  align-items: center;
  padding-left: 15px;
}
.book-list-item-subtitle {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
}
.book-list-item-subtitle-text {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 15px;
  line-height: 18px;
  color: var(--color-text-1);
}
.book-list-item-percentage {
  width: 60px;
  font-size: 13px;
  line-height: 18px;
  text-align: right;
  padding-right: 10px;
  color: var(--color-text-3);
  flex-shrink: 0;
}
.book-list-item-author {
  width: 130px;
  font-size: 13px;
  line-height: 18px;
  text-align: right;
  padding-right: 10px;
  text-overflow: ellipsis;
  overflow: hidden;
  white-space: nowrap;
  color: var(--color-text-3);
  flex-shrink: 0;
}
.book-item-actions {
  display:flex;
  align-items:center;
  gap:4px;
  flex-shrink:0;
  margin-left:8px;
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
.empty-cover-list {
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
