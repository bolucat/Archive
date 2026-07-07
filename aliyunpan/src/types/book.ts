import type { BookViewMode } from './bookShelf'

export interface IBookItem {
  id: string
  user_id: string
  drive_id: string
  file_id: string
  parent_file_id: string
  parent_path?: string
  file_name: string
  ext: string
  size: number
  category: string
  thumbnail?: string
  description?: string
  encType?: string
  title?: string
  author?: string
  summary?: string
  cover_url?: string
  isbn?: string
  publisher?: string
  published_date?: string
  language?: string
  subjects?: string[]
  metadata_source?: string
  metadata_updated_at?: number
  reader_engine?: 'reader'
  is_favorite?: boolean
  shelf_id?: string
  view_mode?: BookViewMode
  reading_position?: Record<string, unknown>
  reading_progress?: number
  reading_progress_text?: string
  reading_chapter?: string
  reading_time?: number
  last_read_at?: number
  deleted_at?: number
  scanned_at: number
  updated_at?: number
}
