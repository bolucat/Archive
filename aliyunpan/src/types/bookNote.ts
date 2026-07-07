import type { BookReaderPosition } from '../utils/bookReaderState'

export type BookNoteKind = 'highlight' | 'note'

export interface IBookNote {
  id: string
  book_id: string
  user_id: string
  drive_id: string
  file_id: string
  kind: BookNoteKind
  text: string
  note: string
  chapter: string
  chapter_index: number
  position: BookReaderPosition
  range: string
  color: number
  tags: string[]
  created_at: number
  updated_at: number
}
