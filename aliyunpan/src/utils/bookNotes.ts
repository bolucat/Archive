import type { IBookItem } from '../types/book'
import type { IBookNote } from '../types/bookNote'
import type { BookReaderPosition } from './bookReaderState'

interface BuildBookHighlightOptions {
  book: Pick<IBookItem, 'id' | 'user_id' | 'drive_id' | 'file_id'>
  text: string
  position: BookReaderPosition
  range: string
  color?: number
  note?: string
  now?: number
}

interface UpdateBookNoteOptions {
  note?: string
  color?: number
  tags?: string[]
  now?: number
}

function normalizeHighlightText(text: string): string {
  return (text || '').replace(/\s+/g, ' ').trim()
}

function readChapterIndex(position: BookReaderPosition): number {
  const numeric = Number(position.chapterDocIndex || 0)
  return Number.isFinite(numeric) ? numeric : 0
}

export function buildBookHighlight(options: BuildBookHighlightOptions): IBookNote {
  const now = options.now || Date.now()
  return {
    id: `${options.book.id}|${now}`,
    book_id: options.book.id,
    user_id: options.book.user_id,
    drive_id: options.book.drive_id,
    file_id: options.book.file_id,
    kind: options.note ? 'note' : 'highlight',
    text: normalizeHighlightText(options.text),
    note: options.note || '',
    chapter: options.position.chapterTitle || '',
    chapter_index: readChapterIndex(options.position),
    position: options.position,
    range: options.range,
    color: options.color ?? 0,
    tags: [],
    created_at: now,
    updated_at: now
  }
}

export function updateBookNote(note: IBookNote, patch: UpdateBookNoteOptions): IBookNote {
  const nextNote = patch.note !== undefined ? patch.note.trim() : note.note
  const nextTags = patch.tags !== undefined
    ? patch.tags.map((tag) => tag.trim()).filter(Boolean)
    : note.tags

  return {
    ...note,
    kind: nextNote ? 'note' : 'highlight',
    note: nextNote,
    color: patch.color ?? note.color,
    tags: nextTags,
    updated_at: patch.now || Date.now()
  }
}

export function toBookNote(note: IBookNote) {
  return {
    key: note.id,
    bookKey: note.book_id,
    date: {
      year: new Date(note.created_at).getFullYear(),
      month: new Date(note.created_at).getMonth() + 1,
      day: new Date(note.created_at).getDate()
    },
    chapter: note.chapter,
    chapterIndex: note.chapter_index,
    text: note.text,
    cfi: JSON.stringify(note.position || {}),
    range: note.range,
    notes: note.note || '',
    percentage: note.position?.percentage !== undefined ? String(note.position.percentage) : '0',
    color: note.color,
    tag: note.tags || []
  }
}
