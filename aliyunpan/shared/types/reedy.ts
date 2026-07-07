export type IndexingStatus = 'not_indexed' | 'indexing' | 'indexed' | 'failed' | 'empty_index'

export interface BookMeta {
  book_hash: string
  indexing_status: IndexingStatus
  chunk_count: number
  embedding_model: string
  embedding_dim: number
  indexed_at: number | null
  error: string | null
}

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

export interface EmbeddingRow {
  chunk_id: string
  book_hash: string
  embedding: Float32Array
}

export interface ScoredChunk {
  chunk: ChunkRow
  score: number
  vectorRank: number | null
  ftsRank: number | null
}

export interface MemoryRow {
  id: string
  scope: MemoryScope
  scope_key: string
  key: string
  summary: string
  source_message_id: string | null
  updated_at: number
}

export type MemoryScope = 'user' | 'book' | 'session'

export interface ConsolidatedMemory {
  scope: MemoryScope
  key: string
  summary: string
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

export type SkillId = 'spoiler-free' | 'chapter-summary' | 'quote-finder'

export interface ReedySettings {
  enabled: boolean
  runtime: 'mvp' | 'agent'
}

export interface IndexProgress {
  phase: 'chunking' | 'embedding'
  current: number
  total: number
}

export type ReedyEventType =
  | 'turn_start'
  | 'text_delta'
  | 'tool_call'
  | 'tool_result'
  | 'citation'
  | 'memory_write'
  | 'step_finish'
  | 'usage'
  | 'error'
  | 'abort'
  | 'done'

export interface ReedyEventBase {
  type: ReedyEventType
  turnId: string
}

export interface TurnStartEvent extends ReedyEventBase {
  type: 'turn_start'
}

export interface TextDeltaEvent extends ReedyEventBase {
  type: 'text_delta'
  delta: string
}

export interface ToolCallEvent extends ReedyEventBase {
  type: 'tool_call'
  toolName: string
  args: Record<string, unknown>
  toolCallId: string
}

export interface ToolResultEvent extends ReedyEventBase {
  type: 'tool_result'
  toolName: string
  toolCallId: string
  result: string
}

export interface CitationEvent extends ReedyEventBase {
  type: 'citation'
  cfi: string
  chapter: string
  text: string
  trust: 'untrusted'
}

export interface MemoryWriteEvent extends ReedyEventBase {
  type: 'memory_write'
  scope: MemoryScope
  key: string
}

export interface StepFinishEvent extends ReedyEventBase {
  type: 'step_finish'
  stepNumber: number
}

export interface UsageEvent extends ReedyEventBase {
  type: 'usage'
  inputTokens: number
  outputTokens: number
}

export interface ErrorEvent extends ReedyEventBase {
  type: 'error'
  code: string
  message: string
}

export interface AbortEvent extends ReedyEventBase {
  type: 'abort'
}

export interface DoneEvent extends ReedyEventBase {
  type: 'done'
}

export type ReedyEvent =
  | TurnStartEvent
  | TextDeltaEvent
  | ToolCallEvent
  | ToolResultEvent
  | CitationEvent
  | MemoryWriteEvent
  | StepFinishEvent
  | UsageEvent
  | ErrorEvent
  | AbortEvent
  | DoneEvent

export interface ReedyIPC {
  'reedy:init': { request: { dbPath: string }; response: { ok: boolean } }
  'reedy:get-meta': { request: { bookHash: string }; response: BookMeta | null }
  'reedy:clear-book': { request: { bookHash: string }; response: { ok: boolean } }
  'reedy:list-skills': { request: {}; response: Skill[] }
  'reedy:toggle-skill': { request: { skillId: string; enabled: boolean }; response: { ok: boolean } }
  'reedy:update-skill-instructions': { request: { skillId: string; instructions: string }; response: { ok: boolean } }
  'reedy:export-metrics': { request: { since?: number }; response: MetricsPayload[] }
  'reedy:destroy': { request: {}; response: { ok: boolean } }
}

export interface MetricsPayload {
  id: number
  ts: number
  event: string
  book_hash: string | null
  session_id: string | null
  turn_id: string | null
  message_id: string | null
  app_version: string
  schema_version: number
  payload: string | null
}

export const RRF_K = 60
export const RRF_FETCH_MULTIPLIER = 3
export const MAX_QUERY_CHARS = 500
export const MAX_TOP_K = 5
export const PER_TURN_BUDGET_MS = 10_000
export const RESULT_SIZE_CAP_CHARS = 6_000
export const DEFAULT_EMBEDDING_TIMEOUT_MS = 5_000
export const DEFAULT_BATCH_SIZE = 16
export const OLLAMA_BATCH_SIZE = 4
export const CHUNK_OPTIONS = {
  maxChunkSize: 500,
  minChunkSize: 100,
  overlapSize: 50,
  breakSearchRange: 50
} as const
export const MEMORY_CONSOLIDATION_THRESHOLD = 6
export const MEMORY_MAX_PER_RUN = 3
export const PROMPT_SAFETY_MARGIN_TOKENS = 256
export const CHAR_PER_TOKEN = 4
