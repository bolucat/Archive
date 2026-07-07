import type { IBookItem } from '../types/book'
import type { IBookBookmark } from '../types/bookBookmark'
import type { IBookNote } from '../types/bookNote'
import type { BookShelfGroup, BookManagerSortMode, BookManagerSortOrder, BookManagerTab, BookManagerView } from '../types/bookShelf'

export const DEFAULT_SHELF_ID = '__default__'
export const DEFAULT_SHELF_NAME = '默认书架'

const LS_NOTE_TAGS = 'bookLibrary.noteTags'

export function loadGlobalNoteTags(): string[] {
  try {
    const raw = localStorage.getItem(LS_NOTE_TAGS)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    return Array.isArray(parsed) ? parsed.map((tag) => String(tag).trim()).filter(Boolean) : []
  } catch {
    return []
  }
}

export function saveGlobalNoteTags(tags: string[]) {
  try { localStorage.setItem(LS_NOTE_TAGS, JSON.stringify(tags)) } catch {}
}

export function addGlobalNoteTag(tag: string): string[] {
  const trimmed = tag.trim()
  if (!trimmed) return loadGlobalNoteTags()
  const tags = loadGlobalNoteTags()
  if (tags.includes(trimmed)) return tags
  const updated = [...tags, trimmed].sort((a, b) => a.localeCompare(b))
  saveGlobalNoteTags(updated)
  return updated
}

export function removeGlobalNoteTag(tag: string): string[] {
  const trimmed = tag.trim()
  if (!trimmed) return loadGlobalNoteTags()
  const tags = loadGlobalNoteTags().filter((item) => item !== trimmed)
  saveGlobalNoteTags(tags)
  return tags
}

export function renameGlobalNoteTag(oldTag: string, newTag: string): string[] {
  const old = oldTag.trim()
  const next = newTag.trim()
  if (!old || !next || old === next) return loadGlobalNoteTags()
  const tags = loadGlobalNoteTags().map((item) => item === old ? next : item)
  saveGlobalNoteTags(tags)
  return tags
}

export function syncGlobalTagsFromNotes(notes: IBookNote[]) {
  const existing = new Set(loadGlobalNoteTags())
  const noteTags = getAnnotationTags(notes)
  let changed = false
  for (const tag of noteTags) {
    if (!existing.has(tag)) {
      existing.add(tag)
      changed = true
    }
  }
  if (changed) saveGlobalNoteTags(Array.from(existing).sort((a, b) => a.localeCompare(b)))
}

export function stripGlobalTagFromNotes(tag: string, notes: IBookNote[]): IBookNote[] {
  const trimmed = tag.trim()
  if (!trimmed) return notes
  return notes.map((note) => {
    const updated = removeAnnotationTag(note, trimmed)
    if ((updated.tags || []).length === (note.tags || []).length) return note
    return { ...updated, updated_at: Date.now() }
  })
}

export type ReadingStatus = '' | 'unread' | 'reading' | 'finished'

export function filterBooksByReadingStatus(books: IBookItem[], status: ReadingStatus): IBookItem[] {
  if (!status) return books
  return books.filter((book) => {
    const progress = book.reading_progress ?? 0
    if (status === 'unread') return progress === 0 || progress === 0
    if (status === 'reading') return progress > 0 && progress < 100
    if (status === 'finished') return progress >= 100
    return true
  })
}

export type ReaderManagerFilterOptions =
  | { view: 'shelf'; shelfId: string }
  | { view: 'folder'; folderPath: string }
  | { view: 'format'; format: string }
  | { view: Exclude<BookManagerView, 'shelf' | 'folder' | 'format'> }

export type BookAnnotationSortMode = 'date' | 'progress'

export function getBookManagerTabs(): BookManagerTab[] {
  return [
    { key: 'home', label: '全部图书' },
    { key: 'recent', label: '最近阅读' },
    { key: 'favorites', label: '收藏' },
    { key: 'shelves', label: '书架' },
    { key: 'notes', label: '笔记' },
    { key: 'highlights', label: '书摘' },
    { key: 'bookmarks', label: '书签' },
    { key: 'trash', label: '已删除' },
    { key: 'folders', label: '文件夹' },
    { key: 'formats', label: '格式' },
    { key: 'stats', label: '阅读统计' }
  ]
}

export function filterBooksForManagerView(
  books: IBookItem[],
  options: ReaderManagerFilterOptions
): IBookItem[] {
  switch (options.view) {
    case 'favorites':
      return books.filter((book) => book.is_favorite)
    case 'recent':
      return books
        .filter((book) => book.last_read_at != null)
        .sort((a, b) => (b.last_read_at ?? 0) - (a.last_read_at ?? 0))
    case 'shelf':
      if (!options.shelfId) return []
      return books.filter((book) => book.shelf_id === options.shelfId)
    case 'folder':
      if (!options.folderPath) return []
      return books.filter((book) => book.parent_path === options.folderPath)
    case 'format':
      if (!options.format) return []
      return books.filter((book) => book.ext.toLowerCase() === (options.format ?? '').toLowerCase())
    default:
      return [...books]
  }
}

export function sortBooksForManagerView(
  books: IBookItem[],
  sortMode: BookManagerSortMode,
  sortOrder?: BookManagerSortOrder
): IBookItem[] {
  const direction = sortOrder || defaultBookSortOrder(sortMode)
  return [...books].sort((a, b) => {
    const ordered = (value: number) => direction === 'asc' ? value : -value
    if (sortMode === 'title') {
      return ordered((a.title || a.file_name).localeCompare(b.title || b.file_name))
    }

    if (sortMode === 'author') {
      return ordered((a.author || '').localeCompare(b.author || ''))
    }

    if (sortMode === 'added') {
      return ordered((a.scanned_at || 0) - (b.scanned_at || 0))
    }

    if (sortMode === 'readingTime') {
      return ordered((a.reading_time || 0) - (b.reading_time || 0))
    }

    if (sortMode === 'progress') {
      return ordered((a.reading_progress || 0) - (b.reading_progress || 0))
    }

    if (sortMode === 'size') {
      return ordered((a.size || 0) - (b.size || 0))
    }

    return ordered((a.last_read_at ?? 0) - (b.last_read_at ?? 0))
  })
}

export function defaultBookSortOrder(sortMode: BookManagerSortMode): BookManagerSortOrder {
  return sortMode === 'title' || sortMode === 'author' ? 'asc' : 'desc'
}

export function normalizeBookSortMode(value: unknown): BookManagerSortMode {
  if (value === 'title' || value === 'author' || value === 'added' || value === 'recent' || value === 'readingTime' || value === 'progress' || value === 'size') {
    return value
  }
  return 'added'
}

export function normalizeBookSortOrder(value: unknown, sortMode: BookManagerSortMode): BookManagerSortOrder {
  return value === 'asc' || value === 'desc' ? value : defaultBookSortOrder(sortMode)
}

export function toggleBookId(ids: string[], id: string): string[] {
  if (ids.includes(id)) {
    return ids.filter((bookId) => bookId !== id)
  }

  return [...ids, id]
}

export function getAnnotationTags(notes: IBookNote[]): string[] {
  const tags = new Set<string>()
  for (const note of notes) {
    for (const tag of note.tags || []) {
      const trimmed = tag.trim()
      if (trimmed) tags.add(trimmed)
    }
  }
  return Array.from(tags).sort((a, b) => a.localeCompare(b))
}

function normalizeTag(tag: string): string {
  return tag.trim()
}

export function addAnnotationTag<T extends Pick<IBookNote, 'tags'>>(note: T, tag: string): T {
  const normalized = normalizeTag(tag)
  if (!normalized) return { ...note, tags: [...(note.tags || [])] }
  const tags = Array.from(new Set([...(note.tags || []).map(normalizeTag).filter(Boolean), normalized]))
  return { ...note, tags }
}

export function renameAnnotationTag<T extends Pick<IBookNote, 'tags'>>(note: T, from: string, to: string): T {
  const source = normalizeTag(from)
  const target = normalizeTag(to)
  if (!source || !target) return { ...note, tags: [...(note.tags || [])] }
  const tags = (note.tags || []).map((tag) => normalizeTag(tag) === source ? target : normalizeTag(tag)).filter(Boolean)
  return { ...note, tags: Array.from(new Set(tags)) }
}

export function removeAnnotationTag<T extends Pick<IBookNote, 'tags'>>(note: T, tag: string): T {
  const normalized = normalizeTag(tag)
  return { ...note, tags: (note.tags || []).map(normalizeTag).filter((item) => item && item !== normalized) }
}

export function filterAnnotations(
  notes: IBookNote[],
  filters: { tags?: string[]; bookId?: string; keyword?: string } = {}
): IBookNote[] {
  const selectedTags = (filters.tags || []).map((tag) => tag.trim()).filter(Boolean)
  const selectedTagSet = new Set(selectedTags)
  const keyword = (filters.keyword || '').trim().toLowerCase()
  return notes.filter((note) => {
    if (filters.bookId && note.book_id !== filters.bookId) return false
    if (keyword && ![
      note.text,
      note.note,
      note.chapter,
      ...(note.tags || [])
    ].some((value) => String(value || '').toLowerCase().includes(keyword))) return false
    if (!selectedTagSet.size) return true
    const noteTagSet = new Set((note.tags || []).map((tag) => tag.trim()).filter(Boolean))
    return selectedTags.every((tag) => noteTagSet.has(tag))
  })
}

export function sortAnnotations(
  notes: IBookNote[],
  sortMode: BookAnnotationSortMode = 'date',
  sortOrder: BookManagerSortOrder = 'desc'
): IBookNote[] {
  const direction = sortOrder === 'asc' ? 1 : -1
  return [...notes].sort((a, b) => {
    const primary = sortMode === 'progress'
      ? annotationProgress(a) - annotationProgress(b)
      : annotationDate(a) - annotationDate(b)
    const fallback = annotationDate(a) - annotationDate(b)
    return direction * (primary || fallback || a.id.localeCompare(b.id))
  })
}

export function filterBookmarks(
  bookmarks: IBookBookmark[],
  keyword?: string
): IBookBookmark[] {
  const q = (keyword || '').trim().toLowerCase()
  if (!q) return [...bookmarks]
  return bookmarks.filter((bookmark) => [
    bookmark.label,
    bookmark.chapter,
    `${Math.round((Number(bookmark.percentage) || 0) * 100)}%`
  ].some((value) => String(value || '').toLowerCase().includes(q)))
}

export function buildNoteLink(noteId: string): string {
  return `reader-reader://open-note?noteKey=${encodeURIComponent(noteId)}`
}

function annotationDate(note: IBookNote): number {
  return Number(note.updated_at || note.created_at || 0) || 0
}

function annotationProgress(note: IBookNote): number {
  return Number(note.position?.percentage) || 0
}

export function buildDefaultShelfName(name: string): string {
  return name.trim() || DEFAULT_SHELF_NAME
}

export function normalizeShelfId(shelfId?: string): string {
  const trimmed = (shelfId || '').trim()
  return trimmed || DEFAULT_SHELF_ID
}

export function shelfNameFromId(shelfId?: string): string {
  const id = normalizeShelfId(shelfId)
  return id === DEFAULT_SHELF_ID ? DEFAULT_SHELF_NAME : id
}

export function buildShelfGroups(books: IBookItem[]): BookShelfGroup[] {
  const groups = new Map<string, { bookIds: string[]; updatedAt: number; createdAt: number }>()
  for (const book of books) {
    const shelfId = normalizeShelfId(book.shelf_id)
    const updatedAt = book.updated_at || book.metadata_updated_at || book.scanned_at || 0
    const createdAt = book.scanned_at || updatedAt
    const current = groups.get(shelfId)
    if (current) {
      current.bookIds.push(book.id)
      current.updatedAt = Math.max(current.updatedAt, updatedAt)
      current.createdAt = Math.min(current.createdAt || createdAt, createdAt)
    } else {
      groups.set(shelfId, { bookIds: [book.id], updatedAt, createdAt })
    }
  }

  return Array.from(groups.entries())
    .map(([id, group]) => ({
      id,
      name: shelfNameFromId(id),
      book_ids: group.bookIds,
      created_at: group.createdAt,
      updated_at: group.updatedAt
    }))
    .sort((a, b) => {
      if (a.id === DEFAULT_SHELF_ID) return -1
      if (b.id === DEFAULT_SHELF_ID) return 1
      return b.updated_at - a.updated_at || a.name.localeCompare(b.name)
    })
}
