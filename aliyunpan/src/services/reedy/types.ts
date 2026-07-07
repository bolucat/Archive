export type IndexingStatus = 'not_indexed' | 'indexing' | 'indexed' | 'failed' | 'empty_index'

export interface ChunkRow {
  id: string
  book_hash: string
  section_index: number
  chapter_title: string | null
  start_cfi: string
  end_cfi: string
  position_index: number
  text: string
  token_count: number
}

export interface BookMeta {
  book_hash: string
  indexing_status: IndexingStatus
  chunk_count: number
  embedding_model: string
  embedding_dim: number
  indexed_at: number | null
  error: string | null
}

export interface ScoredChunk {
  chunk: {
    id: string
    book_hash: string
    section_index: number
    chapter_title: string | null
    start_cfi: string
    end_cfi: string
    position_index: number
    text: string
    token_count: number
  }
  score: number
  vectorRank: number | null
  ftsRank: number | null
}

export type MemoryScope = 'user' | 'book' | 'session'

export interface MemoryRow {
  id: string
  scope: MemoryScope
  scope_key: string
  key: string
  summary: string
  source_message_id: string | null
  updated_at: number
}

export interface Skill {
  id: string
  name: string
  description: string
  instructions: string
  tool_allowlist: string[] | null
  builtin: boolean
  enabled: boolean
}

export const CHUNK_OPTIONS = {
  maxChunkSize: 500,
  minChunkSize: 100,
  overlapSize: 50,
  breakSearchRange: 50
} as const

export const MAX_QUERY_CHARS = 500
export const MAX_TOP_K = 5
