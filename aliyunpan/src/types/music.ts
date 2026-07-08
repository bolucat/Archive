export interface IMusicTrack {
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
  artist?: string
  title?: string
  album?: string
  cover_url?: string
  duration_ms?: number
  lyric_source?: string
  metadata_source?: string
  scanned_at: number
  updated_at?: number
  enriched_at?: number
}
