import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { IBookItem } from '../types/book'
import type { IBookNote } from '../types/bookNote'
import type { IBookBookmark } from '../types/bookBookmark'
import type { BookViewMode, BookManagerSortMode, BookManagerSortOrder } from '../types/bookShelf'
import { updateBookNote as applyBookNotePatch } from '../utils/bookNotes'
import { buildShelfGroups, loadGlobalNoteTags, normalizeBookSortMode, normalizeBookSortOrder, renameAnnotationTag, saveGlobalNoteTags, stripGlobalTagFromNotes, syncGlobalTagsFromNotes } from '../utils/bookManagerParity'
import DB from '../utils/db'

const LS_LASTSCAN = 'bookLibrary.lastScanAt'
const LS_SUBTAB = 'bookLibrary.subTab'
const LS_MANAGER_SORT = 'bookLibrary.readerSort'
const LS_MANAGER_SORT_ORDER = 'bookLibrary.readerSortOrder'
const LS_VIEW_MODE = 'bookLibrary.readerViewMode'

export type BookSubTab = 'shelf' | 'all' | 'authors' | 'formats' | 'folders'

const AUTHOR_TITLE_RE = /^(.+?)\s*[-–—_]\s*(.+)$/
const BRACKET_AUTHOR_RE = /^[\[【（(](.+?)[\]】）)]\s*(.+)$/
const TITLE_AUTHOR_RE = /^(.+?)\s*[\(（](.+?)[\)）]$/
const COMMON_TAGS_RE = /(?:\[[^\]]+\]|【[^】]+】|（[^）]+）|\([^)]+\))/g

function loadJson<T>(key: string, fallback: T): T {
  try {
    const s = localStorage.getItem(key)
    if (!s) return fallback
    const parsed = JSON.parse(s)
    return parsed === null || parsed === undefined ? fallback : parsed
  } catch {
    return fallback
  }
}

function saveJson(key: string, value: unknown) {
  try { localStorage.setItem(key, JSON.stringify(value)) } catch {}
}

function stripExt(name: string): string {
  if (!name) return ''
  const i = name.lastIndexOf('.')
  return i > 0 ? name.slice(0, i) : name
}

export function parseBookMeta(fileName: string): Pick<IBookItem, 'title' | 'author' | 'summary' | 'metadata_source'> {
  const raw = stripExt(fileName).replace(/\s+/g, ' ').trim()
  if (!raw) return { title: fileName || '未命名书籍', author: '未知作者', summary: '', metadata_source: 'unknown' }

  const bracket = raw.match(BRACKET_AUTHOR_RE)
  if (bracket?.[1] && bracket?.[2]) {
    return {
      author: bracket[1].trim(),
      title: bracket[2].replace(COMMON_TAGS_RE, ' ').replace(/\s+/g, ' ').trim(),
      summary: '',
      metadata_source: 'filename'
    }
  }

  const titleAuthor = raw.match(TITLE_AUTHOR_RE)
  if (titleAuthor?.[1] && titleAuthor?.[2] && titleAuthor[2].length <= 40) {
    return {
      title: titleAuthor[1].replace(COMMON_TAGS_RE, ' ').replace(/\s+/g, ' ').trim(),
      author: titleAuthor[2].trim(),
      summary: '',
      metadata_source: 'filename'
    }
  }

  const pair = raw.match(AUTHOR_TITLE_RE)
  if (pair?.[1] && pair?.[2] && pair[1].length <= 40) {
    return {
      author: pair[1].trim(),
      title: pair[2].replace(COMMON_TAGS_RE, ' ').replace(/\s+/g, ' ').trim(),
      summary: '',
      metadata_source: 'filename'
    }
  }

  return {
    title: raw.replace(COMMON_TAGS_RE, ' ').replace(/\s+/g, ' ').trim() || raw,
    author: '未知作者',
    summary: '',
    metadata_source: 'filename'
  }
}

function ensureBookMeta(book: IBookItem): IBookItem {
  if (book.title && book.author && book.summary) return book
  const parsed = parseBookMeta(book.file_name)
  const ext = (book.ext || '').toUpperCase()
  const path = book.parent_path || '根目录'
  return {
    ...book,
    title: book.title || parsed.title || book.file_name,
    author: book.author || parsed.author || '未知作者',
    summary: book.summary || book.description || `${ext || 'BOOK'} · ${path}`,
    metadata_source: book.metadata_source || (book.thumbnail ? 'thumbnail' : parsed.metadata_source)
  }
}

const useBookLibraryStore = defineStore('booklibrary', () => {
  const books = ref<IBookItem[]>([])
  const notesByBookId = ref<Record<string, IBookNote[]>>({})
  const bookmarksByBookId = ref<Record<string, IBookBookmark[]>>({})
  const loaded = ref(false)
  const isScanning = ref(false)
  const scanLabel = ref('')
  const scanScanned = ref(0)
  const scanFound = ref(0)
  const scanError = ref('')
  const lastScanAt = ref<number>(loadJson<number>(LS_LASTSCAN, 0))
  const subTab = ref<BookSubTab>(loadJson<BookSubTab>(LS_SUBTAB, 'shelf'))
  const sortMode = ref<BookManagerSortMode>(normalizeBookSortMode(loadJson<unknown>(LS_MANAGER_SORT, 'added')))
  const sortOrder = ref<BookManagerSortOrder>(normalizeBookSortOrder(loadJson<unknown>(LS_MANAGER_SORT_ORDER, undefined), sortMode.value))
  const viewMode = ref<BookViewMode>(loadJson<BookViewMode>(LS_VIEW_MODE, 'grid'))
  const noteTags = ref<string[]>(loadGlobalNoteTags())

  const activeBooks = computed(() => books.value.filter((book) => !book.deleted_at))
  const deletedBooks = computed(() => books.value.filter((book) => !!book.deleted_at))
  const activeBookIds = computed(() => new Set(activeBooks.value.map((book) => book.id)))
  const totalCount = computed(() => activeBooks.value.length)
  const deletedCount = computed(() => deletedBooks.value.length)
  const allHighlights = computed(() => Object.values(notesByBookId.value)
    .flat()
    .filter((note) => activeBookIds.value.has(note.book_id) && note.kind === 'highlight')
    .sort((a, b) => b.updated_at - a.updated_at))
  const allNotes = computed(() => Object.values(notesByBookId.value)
    .flat()
    .filter((note) => activeBookIds.value.has(note.book_id) && (note.kind === 'note' || !!note.note.trim()))
    .sort((a, b) => b.updated_at - a.updated_at))
  const allBookmarks = computed(() => Object.values(bookmarksByBookId.value)
    .flat()
    .filter((bookmark) => activeBookIds.value.has(bookmark.book_id))
    .sort((a, b) => b.updated_at - a.updated_at))
  const byShelf = computed(() => buildShelfGroups(activeBooks.value))

  const recentlyAdded = computed(() => [...activeBooks.value]
    .sort((a, b) => (b.scanned_at || 0) - (a.scanned_at || 0))
    .slice(0, 36))

  const byAuthor = computed(() => {
    const map = new Map<string, IBookItem[]>()
    for (const b of activeBooks.value) {
      const key = (b.author || '').trim() || '未知作者'
      const arr = map.get(key)
      if (arr) arr.push(b)
      else map.set(key, [b])
    }
    return Array.from(map.entries())
      .map(([author, items]) => ({ author, items, count: items.length }))
      .sort((a, b) => b.count - a.count || a.author.localeCompare(b.author))
  })

  const byFormat = computed(() => {
    const map = new Map<string, IBookItem[]>()
    for (const b of activeBooks.value) {
      const key = (b.ext || 'book').toUpperCase()
      const arr = map.get(key)
      if (arr) arr.push(b)
      else map.set(key, [b])
    }
    return Array.from(map.entries())
      .map(([format, items]) => ({ format, items, count: items.length }))
      .sort((a, b) => b.count - a.count || a.format.localeCompare(b.format))
  })

  const byFolder = computed(() => {
    const map = new Map<string, IBookItem[]>()
    for (const b of activeBooks.value) {
      const path = (b.parent_path || b.parent_file_id || '').trim() || '未分组'
      const key = `${b.user_id || ''}|${b.drive_id || ''}|${path}`
      const arr = map.get(key)
      if (arr) arr.push(b)
      else map.set(key, [b])
    }
    return Array.from(map.values())
      .map((items) => {
        const first = items[0]
        const path = (first.parent_path || first.parent_file_id || '').trim() || '未分组'
        return {
          key: `${first.user_id || ''}|${first.drive_id || ''}|${path}`,
          path,
          name: path.split('/').pop() || path,
          user_id: first.user_id || '',
          drive_id: first.drive_id || '',
          items,
          count: items.length,
          scanned_at: items.reduce((m, b) => Math.max(m, b.scanned_at || 0), 0)
        }
      })
      .sort((a, b) => b.scanned_at - a.scanned_at)
  })

  async function loadFromDB() {
    if (loaded.value) return
    try {
      const list = await DB.getAllBookItems()
      const fixed = list.map(ensureBookMeta)
      books.value = fixed
      loaded.value = true
      if (fixed.length) DB.saveBookItems(fixed).catch(() => {})
    } catch (e) {
      console.warn('bookLibrary loadFromDB failed', e)
    }
  }

  function setScanProgress(label: string, scanned: number, found: number) {
    scanLabel.value = label
    scanScanned.value = scanned
    scanFound.value = found
  }

  async function appendBooks(newBooks: IBookItem[]) {
    if (!newBooks.length) return
    const existingById = new Map(books.value.map((book) => [book.id, book]))
    const normalized = newBooks
      .map(ensureBookMeta)
      .filter((book) => !existingById.get(book.id)?.deleted_at)
    if (!normalized.length) return
    await DB.saveBookItems(normalized).catch(() => {})
    const map = new Map<string, IBookItem>()
    for (const b of books.value) map.set(b.id, b)
    for (const b of normalized) map.set(b.id, b)
    books.value = Array.from(map.values())
  }

  async function updateBookMetadata(id: string, patch: Partial<IBookItem>) {
    const idx = books.value.findIndex((b) => b.id === id)
    if (idx < 0) return
    const merged: IBookItem = {
      ...books.value[idx],
      ...patch,
      id: books.value[idx].id,
      metadata_updated_at: Date.now()
    }
    books.value = [
      ...books.value.slice(0, idx),
      merged,
      ...books.value.slice(idx + 1)
    ]
    DB.saveBookItems([merged]).catch(() => {})
  }

  async function toggleFavoriteBook(id: string) {
    const book = books.value.find((item) => item.id === id)
    if (!book) return
    await updateBookMetadata(id, { is_favorite: !book.is_favorite })
  }

  async function moveBooksToShelf(ids: string[], shelfId: string) {
    const idSet = new Set(ids)
    const updatedAt = Date.now()
    const changed = books.value.filter((book) => idSet.has(book.id)).map((book) => ({
      ...book,
      shelf_id: shelfId,
      metadata_updated_at: updatedAt
    }))
    if (!changed.length) return
    await DB.saveBookItems(changed).catch(() => {})
    books.value = books.value.map((book) => {
      if (!idSet.has(book.id)) return book
      return {
        ...book,
        shelf_id: shelfId,
        metadata_updated_at: updatedAt
      }
    })
  }

  function removeBooksByIds(ids: string[]) {
    if (!ids || ids.length === 0) return
    const removeSet = new Set(ids)
    books.value = books.value.filter((b) => !removeSet.has(b.id))
  }

  async function deleteBooksByIds(ids: string[]) {
    if (!ids || ids.length === 0) return 0
    await Promise.all([
      DB.deleteBookNotesByBookIds(ids).catch(() => 0),
      DB.deleteBookBookmarksByBookIds(ids).catch(() => 0)
    ])
    const deleted = await DB.deleteBookItemsByIds(ids).catch(() => 0)
    const idSet = new Set(ids)
    notesByBookId.value = Object.fromEntries(Object.entries(notesByBookId.value).filter(([bookId]) => !idSet.has(bookId)))
    bookmarksByBookId.value = Object.fromEntries(Object.entries(bookmarksByBookId.value).filter(([bookId]) => !idSet.has(bookId)))
    removeBooksByIds(ids)
    return deleted || ids.length
  }

  async function moveBooksToTrash(ids: string[]) {
    if (!ids || ids.length === 0) return 0
    const idSet = new Set(ids)
    const deletedAt = Date.now()
    const changed = books.value.filter((book) => idSet.has(book.id) && !book.deleted_at).map((book) => ({
      ...book,
      deleted_at: deletedAt,
      is_favorite: false,
      metadata_updated_at: deletedAt
    }))
    if (!changed.length) return 0
    await DB.saveBookItems(changed).catch(() => {})
    books.value = books.value.map((book) => {
      if (!idSet.has(book.id) || book.deleted_at) return book
      return {
        ...book,
        deleted_at: deletedAt,
        is_favorite: false,
        metadata_updated_at: deletedAt
      }
    })
    return changed.length
  }

  async function restoreBooksFromTrash(ids: string[]) {
    if (!ids || ids.length === 0) return 0
    const idSet = new Set(ids)
    const updatedAt = Date.now()
    const changed = books.value.filter((book) => idSet.has(book.id) && book.deleted_at).map((book) => ({
      ...book,
      deleted_at: undefined,
      metadata_updated_at: updatedAt
    }))
    if (!changed.length) return 0
    await DB.saveBookItems(changed).catch(() => {})
    books.value = books.value.map((book) => {
      if (!idSet.has(book.id) || !book.deleted_at) return book
      return {
        ...book,
        deleted_at: undefined,
        metadata_updated_at: updatedAt
      }
    })
    return changed.length
  }

  async function loadNotesByBookId(bookId: string): Promise<IBookNote[]> {
    if (!bookId) return []
    const list = await DB.getBookNotesByBookId(bookId).catch(() => [])
    notesByBookId.value = {
      ...notesByBookId.value,
      [bookId]: list
    }
    return list
  }

  async function loadAllBookAnnotations(): Promise<void> {
    const ids = activeBookIds.value
    const [notes, bookmarks] = await Promise.all([
      DB.getAllBookNotes().catch(() => []),
      DB.getAllBookBookmarks().catch(() => [])
    ])
    const nextNotes: Record<string, IBookNote[]> = {}
    for (const note of notes) {
      if (!ids.has(note.book_id)) continue
      nextNotes[note.book_id] = [...(nextNotes[note.book_id] || []), note]
    }
    const nextBookmarks: Record<string, IBookBookmark[]> = {}
    for (const bookmark of bookmarks) {
      if (!ids.has(bookmark.book_id)) continue
      nextBookmarks[bookmark.book_id] = [...(nextBookmarks[bookmark.book_id] || []), bookmark]
    }
    notesByBookId.value = nextNotes
    bookmarksByBookId.value = nextBookmarks
  }

  async function appendBookNote(note: IBookNote) {
    await DB.saveBookNotes([note]).catch(() => {})
    const current = notesByBookId.value[note.book_id] || []
    notesByBookId.value = {
      ...notesByBookId.value,
      [note.book_id]: [...current.filter((item) => item.id !== note.id), note]
    }
    syncGlobalTagsFromNotes([note])
    noteTags.value = loadGlobalNoteTags()
  }

  async function loadBookmarksByBookId(bookId: string): Promise<IBookBookmark[]> {
    if (!bookId) return []
    const list = await DB.getBookBookmarksByBookId(bookId).catch(() => [])
    bookmarksByBookId.value = {
      ...bookmarksByBookId.value,
      [bookId]: list
    }
    return list
  }

  async function appendBookBookmark(bookmark: IBookBookmark) {
    await DB.saveBookBookmarks([bookmark]).catch(() => {})
    const current = bookmarksByBookId.value[bookmark.book_id] || []
    bookmarksByBookId.value = {
      ...bookmarksByBookId.value,
      [bookmark.book_id]: [...current.filter((item) => item.id !== bookmark.id), bookmark]
    }
  }

  async function deleteBookBookmarksByIds(bookId: string, ids: string[]) {
    if (!bookId || !ids.length) return 0
    const removeSet = new Set(ids)
    const current = bookmarksByBookId.value[bookId] || []
    const deleted = await DB.deleteBookBookmarksByIds(ids).catch(() => 0)
    bookmarksByBookId.value = {
      ...bookmarksByBookId.value,
      [bookId]: current.filter((bookmark) => !removeSet.has(bookmark.id))
    }
    return deleted || ids.length
  }

  async function updateBookNote(noteId: string, patch: { note?: string; color?: number; tags?: string[] }): Promise<IBookNote | null> {
    for (const [bookId, list] of Object.entries(notesByBookId.value)) {
      const idx = list.findIndex((note) => note.id === noteId)
      if (idx < 0) continue
      const updated = applyBookNotePatch(list[idx], patch)
      await DB.saveBookNotes([updated]).catch(() => {})
      notesByBookId.value = {
        ...notesByBookId.value,
        [bookId]: [
          ...list.slice(0, idx),
          updated,
          ...list.slice(idx + 1)
        ]
      }
      return updated
    }
    return null
  }

  async function deleteBookNotesByIds(bookId: string, ids: string[]) {
    if (!bookId || !ids.length) return 0
    const removeSet = new Set(ids)
    const current = notesByBookId.value[bookId] || []
    const deleted = await DB.deleteBookNotesByIds(ids).catch(() => 0)
    notesByBookId.value = {
      ...notesByBookId.value,
      [bookId]: current.filter((note) => !removeSet.has(note.id))
    }
    return deleted || ids.length
  }

  async function deleteAllBookNotesByBookId(bookId: string) {
    await DB.deleteBookNotesByBookIds([bookId]).catch(() => {})
    notesByBookId.value = {
      ...notesByBookId.value,
      [bookId]: []
    }
  }

  async function deleteAllBookBookmarksByBookId(bookId: string) {
    await DB.deleteBookBookmarksByBookIds([bookId]).catch(() => {})
    bookmarksByBookId.value = {
      ...bookmarksByBookId.value,
      [bookId]: []
    }
  }

  function addNoteTag(tag: string) {
    const trimmed = tag.trim()
    if (!trimmed || noteTags.value.includes(trimmed)) return
    noteTags.value = [...noteTags.value, trimmed].sort((a, b) => a.localeCompare(b))
    saveGlobalNoteTags(noteTags.value)
  }

  async function removeNoteTag(tag: string): Promise<number> {
    if (!tag.trim()) return 0
    noteTags.value = noteTags.value.filter((item) => item !== tag.trim())
    saveGlobalNoteTags(noteTags.value)
    const affectedBooks = new Map<string, IBookNote[]>()
    for (const [bookId, list] of Object.entries(notesByBookId.value)) {
      const updated = stripGlobalTagFromNotes(tag, list)
      if (updated.some((note, i) => note !== list[i])) {
        affectedBooks.set(bookId, updated)
      }
    }
    if (!affectedBooks.size) return 0
    const allUpdated = Array.from(affectedBooks.values()).flat()
    await DB.saveBookNotes(allUpdated).catch(() => {})
    for (const [bookId, updated] of affectedBooks) {
      notesByBookId.value = { ...notesByBookId.value, [bookId]: updated }
    }
    return allUpdated.length
  }

  async function renameNoteTag(oldTag: string, newTag: string): Promise<number> {
    const old = oldTag.trim()
    const next = newTag.trim()
    if (!old || !next || old === next || !noteTags.value.includes(old)) return 0
    noteTags.value = noteTags.value.map((item) => item === old ? next : item)
    saveGlobalNoteTags(noteTags.value)
    const affectedBooks = new Map<string, IBookNote[]>()
    for (const [bookId, list] of Object.entries(notesByBookId.value)) {
      const updated = list.map((note) => {
        const current = note.tags || []
        if (!current.includes(old)) return note
        return { ...renameAnnotationTag(note, old, next), updated_at: Date.now() }
      })
      if (updated.some((note, i) => note !== list[i])) {
        affectedBooks.set(bookId, updated)
      }
    }
    if (!affectedBooks.size) return 0
    const allUpdated = Array.from(affectedBooks.values()).flat()
    await DB.saveBookNotes(allUpdated).catch(() => {})
    for (const [bookId, updated] of affectedBooks) {
      notesByBookId.value = { ...notesByBookId.value, [bookId]: updated }
    }
    return allUpdated.length
  }

  function setIsScanning(v: boolean, errMsg = '') {
    isScanning.value = v
    if (!v) {
      scanLabel.value = ''
      scanScanned.value = 0
      scanFound.value = 0
    }
    scanError.value = errMsg
  }

  function markScanFinished() {
    lastScanAt.value = Date.now()
    saveJson(LS_LASTSCAN, lastScanAt.value)
  }

  function setSubTab(t: BookSubTab) {
    subTab.value = t
    saveJson(LS_SUBTAB, t)
  }

  function setSortMode(mode: BookManagerSortMode) {
    const normalized = normalizeBookSortMode(mode)
    sortMode.value = normalized
    sortOrder.value = normalizeBookSortOrder(sortOrder.value, normalized)
    saveJson(LS_MANAGER_SORT, normalized)
    saveJson(LS_MANAGER_SORT_ORDER, sortOrder.value)
  }

  function setSortOrder(order: BookManagerSortOrder) {
    sortOrder.value = normalizeBookSortOrder(order, sortMode.value)
    saveJson(LS_MANAGER_SORT_ORDER, sortOrder.value)
  }

  function setViewMode(mode: BookViewMode) {
    viewMode.value = mode
    saveJson(LS_VIEW_MODE, mode)
  }

  async function clearAll() {
    await DB.clearBookItems().catch(() => {})
    books.value = []
    notesByBookId.value = {}
    bookmarksByBookId.value = {}
    lastScanAt.value = 0
    saveJson(LS_LASTSCAN, 0)
  }

  return {
    books,
    activeBooks,
    deletedBooks,
    notesByBookId,
    bookmarksByBookId,
    loaded,
    isScanning,
    scanLabel,
    scanScanned,
    scanFound,
    scanError,
    lastScanAt,
    subTab,
    sortMode,
    sortOrder,
    viewMode,
    noteTags,
    totalCount,
    deletedCount,
    allHighlights,
    allNotes,
    allBookmarks,
    byShelf,
    recentlyAdded,
    byAuthor,
    byFormat,
    byFolder,
    loadFromDB,
    setScanProgress,
    appendBooks,
    updateBookMetadata,
    loadNotesByBookId,
    loadAllBookAnnotations,
    appendBookNote,
    loadBookmarksByBookId,
    appendBookBookmark,
    deleteBookBookmarksByIds,
    updateBookNote,
    deleteBookNotesByIds,
    deleteAllBookNotesByBookId,
    deleteAllBookBookmarksByBookId,
    removeBooksByIds,
    deleteBooksByIds,
    moveBooksToTrash,
    restoreBooksFromTrash,
    toggleFavoriteBook,
    moveBooksToShelf,
    setIsScanning,
    markScanFinished,
    setSubTab,
    setSortMode,
    setSortOrder,
    setViewMode,
    addNoteTag,
    removeNoteTag,
    renameNoteTag,
    clearAll
  }
})

export default useBookLibraryStore
