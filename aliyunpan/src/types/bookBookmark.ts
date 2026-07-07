import type { BookReaderPosition } from '../utils/bookReaderState'

export interface IBookBookmark {
  id: string
  book_id: string
  user_id: string
  drive_id: string
  file_id: string
  label: string
  chapter: string
  position: BookReaderPosition
  percentage: number
  created_at: number
  updated_at: number
}
