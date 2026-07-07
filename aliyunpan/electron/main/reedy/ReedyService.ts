import { app } from 'electron'
import { join } from 'path'
import { ReedyDb } from './db/ReedyDb'
import type { BookMeta, IndexingStatus, Skill, MemoryScope, ScoredChunk } from '@shared/types/reedy'

let db: ReedyDb | null = null

export function getDb(): ReedyDb {
  if (!db) {
    const appVersion = app.getVersion()
    const dbPath = join(app.getPath('userData'), 'reedy.db')
    db = new ReedyDb(dbPath)
    seedSkills()
  }
  return db
}

export function destroyDb(): void {
  if (db) {
    db.close()
    db = null
  }
}

// ---------------------------------------------------------------------------
// book meta
// ---------------------------------------------------------------------------

export function getBookMeta(bookHash: string): BookMeta | null {
  console.log('[Reedy][Service] getBookMeta:', bookHash)
  return getDb().getMeta(bookHash)
}

export function isBookIndexed(bookHash: string): boolean {
  const result = getDb().isIndexed(bookHash)
  console.log('[Reedy][Service] isBookIndexed:', bookHash, result)
  return result
}

// ---------------------------------------------------------------------------
// index + search
// ---------------------------------------------------------------------------

export function clearBookData(bookHash: string): void {
  getDb().dropBookData(bookHash)
}

export function storeChunks(chunks: Array<{
  id: string
  book_hash: string
  section_index: number
  chapter_title: string | null
  start_cfi: string
  end_cfi: string
  position_index: number
  text: string
  token_count: number
}>): void {
  getDb().insertChunks(chunks)
}

export function storeEmbeddings(rows: Array<{
  chunk_id: string
  book_hash: string
  embedding: Float32Array
}>): void {
  getDb().insertEmbeddings(rows)
}

export function storeMeta(meta: BookMeta): void {
  getDb().upsertMeta(meta)
}

export function hybridSearch(
  bookHash: string,
  queryEmbedding: Float32Array | number[],
  queryText: string,
  topK: number,
  spoilerBoundPosition?: number
): ScoredChunk[] {
  console.log('[Reedy][Service] hybridSearch:', { bookHash, queryText: queryText.slice(0, 80), topK, spoilerBoundPosition, hasEmbedding: queryEmbedding && (queryEmbedding as any).length > 0 })
  const results = getDb().hybridSearch(bookHash, queryEmbedding, queryText, topK, spoilerBoundPosition)
  console.log('[Reedy][Service] hybridSearch results:', results.length, 'first score:', results[0]?.score)
  return results
}

// ---------------------------------------------------------------------------
// memory
// ---------------------------------------------------------------------------

export function writeMemory(args: {
  scope: MemoryScope
  scope_key: string
  key: string
  summary: string
  source_message_id?: string
  embedding?: Float32Array | number[]
}) {
  return getDb().upsertMemory(args)
}

export function searchMemories(
  scope: MemoryScope,
  scopeKey: string,
  queryEmbedding: Float32Array | number[] | null,
  topK: number,
  recencyWeight?: number
) {
  return getDb().searchMemories(scope, scopeKey, queryEmbedding, topK, recencyWeight)
}

export function listMemories(scope: MemoryScope, scopeKey: string, limit: number) {
  return getDb().listMemories(scope, scopeKey, limit)
}

export function deleteMemory(id: string): boolean {
  return getDb().deleteMemory(id)
}

// ---------------------------------------------------------------------------
// skills
// ---------------------------------------------------------------------------

export function listSkills() {
  return getDb().listSkills()
}

export function getSkill(id: string) {
  return getDb().getSkill(id)
}

export function setSkillEnabled(id: string, enabled: boolean) {
  getDb().setSkillEnabled(id, enabled)
}

// ---------------------------------------------------------------------------
// metrics
// ---------------------------------------------------------------------------

export function recordMetric(event: {
  ts: number
  event: string
  book_hash?: string
  session_id?: string
  turn_id?: string
  message_id?: string
  payload?: string
}) {
  getDb().insertMetric({
    ...event,
    app_version: app.getVersion(),
    schema_version: 1
  })
}

export function getMetrics(since?: number) {
  return getDb().getMetrics(since ?? 0)
}

// ---------------------------------------------------------------------------
// wipe
// ---------------------------------------------------------------------------

export function wipeAllData() {
  getDb().wipeAllData()
}

// ---------------------------------------------------------------------------
// skills seed
// ---------------------------------------------------------------------------

const BUILTIN_SKILLS = [
  {
    id: 'spoiler-free',
    name: '防剧透',
    description: '仅基于已读内容回答，不透露后续情节',
    instructions: `你是防剧透助手。用户当前阅读位置由 getReadingContext 提供。
请使用该位置作为查询边界，仅检索用户已阅读的内容。
如果用户的提问必然涉及剧透内容，请礼貌告知用户无法回答，并建议关闭防剧透模式。
即便如此，你仍应该尽力提供基于已读内容的帮助。`,
    tool_allowlist: null,
    builtin: true,
    enabled: true
  },
  {
    id: 'chapter-summary',
    name: '章末总结',
    description: '简洁总结当前章节内容，附带引用',
    instructions: `任务：为当前章节生成简洁的 3-5 句话总结。
步骤：
1. 调用 getReadingContext 获取章节标题和当前阅读位置。
2. 调用 lookupPassage 检索关键情节节点（每次 topK=5）。
3. 综合检索结果，用 3-5 句话概括本章内容，为每个要点添加 CFI 引用。
请严格基于检索到的内容，不要编造。`,
    tool_allowlist: ['getReadingContext', 'lookupPassage', 'addCitation'],
    builtin: true,
    enabled: true
  },
  {
    id: 'quote-finder',
    name: '寻找引文',
    description: '搜索与描述匹配的段落，保持原文引用并附 CFI 引用',
    instructions: `任务：根据用户的描述查找匹配的原文段落。
步骤：
1. 如果用户有选中的文本，通过 getSelection 获取作为种子。
2. 调用 lookupPassage（topK=5）检索相关段落。
3. 返回每一个相关段落，附上相关度说明。不要过度过滤，将判断权交给用户。
4. 严格保持原文引用，不要改写或翻译。`,
    tool_allowlist: ['getReadingContext', 'getSelection', 'lookupPassage', 'addCitation'],
    builtin: true,
    enabled: true
  }
]

function seedSkills(): void {
  for (const skill of BUILTIN_SKILLS) {
    const existing = getDb().getSkill(skill.id)
    if (!existing) {
      getDb().upsertSkill(skill)
    }
  }
}
