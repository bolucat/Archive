import type { IBookItem } from '../types/book'
import type { IBookBookmark } from '../types/bookBookmark'
import type { BookReaderPosition } from './bookReaderState'

interface BuildBookBookmarkOptions {
  book: Pick<IBookItem, 'id' | 'user_id' | 'drive_id' | 'file_id'>
  position: BookReaderPosition
  visibleText?: string
  now?: number
}

function normalizeLabel(text: string): string {
  return (text || '').replace(/\s+/g, ' ').trim().slice(0, 200)
}

export function getBookmarkFingerprint(position: BookReaderPosition): string {
  if (position.cfi) return String(position.cfi)
  if (position.percentage !== undefined) return String(position.percentage)
  return JSON.stringify({
    chapterDocIndex: position.chapterDocIndex || '',
    chapterTitle: position.chapterTitle || '',
    count: position.count || ''
  })
}

export function buildBookBookmark(options: BuildBookBookmarkOptions): IBookBookmark {
  const now = options.now || Date.now()
  const percentage = Number(options.position.percentage || 0)
  const label = normalizeLabel(String(options.position.text || options.visibleText || options.position.chapterTitle || '书签'))
  const fingerprint = getBookmarkFingerprint(options.position)
  return {
    id: `${options.book.id}|${fingerprint}`,
    book_id: options.book.id,
    user_id: options.book.user_id,
    drive_id: options.book.drive_id,
    file_id: options.book.file_id,
    label: label || '书签',
    chapter: options.position.chapterTitle || '',
    position: options.position,
    percentage: Number.isFinite(percentage) ? percentage : 0,
    created_at: now,
    updated_at: now
  }
}
