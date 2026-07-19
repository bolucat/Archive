import Database from 'better-sqlite3'
import type { BookMeta, ChunkRow, EmbeddingRow, ScoredChunk, MemoryRow, MemoryScope, IndexingStatus } from '@shared/types/reedy'
import { RRF_K, RRF_FETCH_MULTIPLIER } from '@shared/types/reedy'
import { deserializeEmbedding, serializeEmbedding, cosineSimilarity } from '@shared/utils/vector'
import { applyMigrations } from './migrations'
import { v4 as uuidv4 } from 'uuid'

export class ReedyDb {
  private db: Database.Database

  constructor(dbPath: string) {
    this.db = new Database(dbPath)
    applyMigrations(this.db)
  }

  close(): void {
    this.db.close()
  }

  private get stmt(): Record<string, Database.Statement> {
    const db = this.db
    if (!this._cached) {
      this._cached = {
        upsertMeta: db.prepare(`
          INSERT INTO reedy_book_meta (book_hash, indexing_status, chunk_count, embedding_model, embedding_dim, indexed_at, error)
          VALUES (@book_hash, @indexing_status, @chunk_count, @embedding_model, @embedding_dim, @indexed_at, @error)
          ON CONFLICT(book_hash) DO UPDATE SET
            indexing_status = excluded.indexing_status,
            chunk_count     = excluded.chunk_count,
            embedding_model = excluded.embedding_model,
            embedding_dim   = excluded.embedding_dim,
            indexed_at      = excluded.indexed_at,
            error           = excluded.error
        `),
        getMeta: db.prepare('SELECT * FROM reedy_book_meta WHERE book_hash = ?'),
        insertChunk: db.prepare(`
          INSERT INTO reedy_book_chunks (id, book_hash, section_index, chapter_title, start_cfi, end_cfi, position_index, text, token_count)
          VALUES (@id, @book_hash, @section_index, @chapter_title, @start_cfi, @end_cfi, @position_index, @text, @token_count)
          ON CONFLICT(id) DO UPDATE SET
            chapter_title = excluded.chapter_title,
            start_cfi     = excluded.start_cfi,
            end_cfi       = excluded.end_cfi,
            text          = excluded.text,
            token_count   = excluded.token_count
        `),
        insertEmbedding: db.prepare(`
          INSERT INTO reedy_book_chunk_embeddings (chunk_id, book_hash, embedding)
          VALUES (@chunk_id, @book_hash, @embedding)
          ON CONFLICT(chunk_id) DO UPDATE SET embedding = excluded.embedding
        `),
        getChunks: db.prepare('SELECT * FROM reedy_book_chunks WHERE book_hash = ? ORDER BY position_index ASC'),
        getChunkEmbeddings: db.prepare('SELECT chunk_id, book_hash, embedding FROM reedy_book_chunk_embeddings WHERE book_hash = ?'),
        deleteChunks: db.prepare('DELETE FROM reedy_book_chunks WHERE book_hash = ?'),
        deleteEmbeddings: db.prepare('DELETE FROM reedy_book_chunk_embeddings WHERE book_hash = ?'),
        deleteMeta: db.prepare('DELETE FROM reedy_book_meta WHERE book_hash = ?'),
        upsertMemory: db.prepare(`
          INSERT INTO reedy_memory (id, scope, scope_key, key, summary, source_message_id, updated_at)
          VALUES (@id, @scope, @scope_key, @key, @summary, @source_message_id, @updated_at)
          ON CONFLICT(scope, scope_key, key) DO UPDATE SET
            summary           = excluded.summary,
            source_message_id = excluded.source_message_id,
            updated_at        = excluded.updated_at
        `),
        insertMemoryEmbedding: db.prepare(`
          INSERT INTO reedy_memory_embeddings (memory_id, embedding)
          VALUES (@memory_id, @embedding)
          ON CONFLICT(memory_id) DO UPDATE SET embedding = excluded.embedding
        `),
        deleteMemory: db.prepare('DELETE FROM reedy_memory WHERE id = ?'),
        deleteMemoryEmbedding: db.prepare('DELETE FROM reedy_memory_embeddings WHERE memory_id = ?'),
        listMemories: db.prepare('SELECT * FROM reedy_memory WHERE scope = ? AND scope_key = ? ORDER BY updated_at DESC LIMIT ?'),
        getMemoryByKey: db.prepare('SELECT * FROM reedy_memory WHERE scope = ? AND scope_key = ? AND key = ?'),
        upsertSkill: db.prepare(`
          INSERT INTO reedy_skills (id, name, description, instructions, tool_allowlist, builtin, enabled)
          VALUES (@id, @name, @description, @instructions, @tool_allowlist, @builtin, @enabled)
          ON CONFLICT(id) DO UPDATE SET
            name           = excluded.name,
            description    = excluded.description,
            instructions   = excluded.instructions,
            tool_allowlist = excluded.tool_allowlist
        `),
        listSkills: db.prepare('SELECT * FROM reedy_skills ORDER BY id'),
        getSkill: db.prepare('SELECT * FROM reedy_skills WHERE id = ?'),
        updateSkillEnabled: db.prepare('UPDATE reedy_skills SET enabled = @enabled WHERE id = @id'),
        insertMetric: db.prepare(`
          INSERT INTO reedy_metrics (ts, event, book_hash, session_id, turn_id, message_id, app_version, schema_version, payload)
          VALUES (@ts, @event, @book_hash, @session_id, @turn_id, @message_id, @app_version, @schema_version, @payload)
        `),
        getMetrics: db.prepare('SELECT * FROM reedy_metrics WHERE ts >= ? ORDER BY ts DESC'),
        wipeAll: db.transaction(() => {
          db.exec('DELETE FROM reedy_book_meta')
          db.exec('DELETE FROM reedy_book_chunks')
          db.exec('DELETE FROM reedy_book_chunk_embeddings')
          db.exec('DELETE FROM reedy_memory')
          db.exec('DELETE FROM reedy_memory_embeddings')
          db.exec('DELETE FROM reedy_skills')
          db.exec('DELETE FROM reedy_metrics')
          db.exec('DELETE FROM reedy_book_chunks_fts')
        }),
      }
    }
    return this._cached
  }
  private _cached: Record<string, Database.Statement> | undefined

  // ---------------------------------------------------------------------------
  // book meta
  // ---------------------------------------------------------------------------

  upsertMeta(meta: BookMeta): void {
    const s = this.stmt.upsertMeta
    s.run({
      book_hash: meta.book_hash,
      indexing_status: meta.indexing_status,
      chunk_count: meta.chunk_count,
      embedding_model: meta.embedding_model,
      embedding_dim: meta.embedding_dim,
      indexed_at: meta.indexed_at ?? null,
      error: meta.error ?? null
    })
  }

  getMeta(bookHash: string): BookMeta | null {
    const row = this.stmt.getMeta.get(bookHash) as Record<string, unknown> | undefined
    if (!row) return null
    return {
      book_hash: row.book_hash as string,
      indexing_status: row.indexing_status as IndexingStatus,
      chunk_count: row.chunk_count as number,
      embedding_model: row.embedding_model as string,
      embedding_dim: row.embedding_dim as number,
      indexed_at: row.indexed_at as number | null,
      error: row.error as string | null
    }
  }

  isIndexed(bookHash: string): boolean {
    const meta = this.getMeta(bookHash)
    return meta !== null && meta.indexing_status === 'indexed'
  }

  // ---------------------------------------------------------------------------
  // chunks
  // ---------------------------------------------------------------------------

  insertChunks(chunks: ChunkRow[]): void {
    const insert = this.stmt.insertChunk
    const insertAll = this.db.transaction((items: ChunkRow[]) => {
      for (const c of items) {
        insert.run({
          id: c.id,
          book_hash: c.book_hash,
          section_index: c.section_index,
          chapter_title: c.chapter_title ?? null,
          start_cfi: c.start_cfi,
          end_cfi: c.end_cfi,
          position_index: c.position_index,
          text: c.text,
          token_count: c.token_count
        })
      }
    })
    insertAll(chunks)
    this.rebuildFts()
  }

  getChunks(bookHash: string): ChunkRow[] {
    const rows = this.stmt.getChunks.all(bookHash) as Record<string, unknown>[]
    return rows.map(r => ({
      id: r.id as string,
      book_hash: r.book_hash as string,
      section_index: r.section_index as number,
      chapter_title: r.chapter_title as string | null,
      start_cfi: r.start_cfi as string,
      end_cfi: r.end_cfi as string,
      position_index: r.position_index as number,
      text: r.text as string,
      token_count: r.token_count as number
    }))
  }

  // ---------------------------------------------------------------------------
  // embeddings
  // ---------------------------------------------------------------------------

  insertEmbeddings(rows: EmbeddingRow[]): void {
    const insert = this.stmt.insertEmbedding
    const insertAll = this.db.transaction((items: EmbeddingRow[]) => {
      for (const r of items) {
        insert.run({
          chunk_id: r.chunk_id,
          book_hash: r.book_hash,
          embedding: serializeEmbedding(r.embedding)
        })
      }
    })
    insertAll(rows)
  }

  getChunkEmbeddings(bookHash: string): Array<{ chunk_id: string; book_hash: string; embedding: Float32Array }> {
    const rows = this.stmt.getChunkEmbeddings.all(bookHash) as Array<{ chunk_id: string; book_hash: string; embedding: Buffer }>
    return rows.map(r => ({
      chunk_id: r.chunk_id,
      book_hash: r.book_hash,
      embedding: deserializeEmbedding(r.embedding)
    }))
  }

  rebuildFts(): void {
    this.db.exec("INSERT INTO reedy_book_chunks_fts(reedy_book_chunks_fts) VALUES('rebuild')")
  }

  // ---------------------------------------------------------------------------
  // data lifecycle
  // ---------------------------------------------------------------------------

  clearBookChunks(bookHash: string): void {
    this.stmt.deleteEmbeddings.run(bookHash)
    this.stmt.deleteChunks.run(bookHash)
    this.rebuildFts()
  }

  dropBookData(bookHash: string): void {
    this.stmt.deleteEmbeddings.run(bookHash)
    this.stmt.deleteChunks.run(bookHash)
    this.stmt.deleteMeta.run(bookHash)
    this.rebuildFts()
  }

  wipeAllData(): void {
    this.stmt.wipeAll()
  }

  // ---------------------------------------------------------------------------
  // hybrid search (vector cosine + FTS5 BM25 + RRF)
  // ---------------------------------------------------------------------------

  hybridSearch(
    bookHash: string,
    queryEmbedding: Float32Array | number[],
    queryText: string,
    topK: number,
    spoilerBoundPosition?: number
  ): ScoredChunk[] {
    if (topK <= 0) return []

    console.log('[Reedy][Db] hybridSearch internal:', { bookHash, queryText: queryText.slice(0, 80), topK, spoilerBoundPosition })

    const embedding = queryEmbedding instanceof Float32Array ? queryEmbedding : new Float32Array(queryEmbedding as number[])
    const fetchK = Math.max(topK, topK * RRF_FETCH_MULTIPLIER)

    // Vector path: brute-force cosine similarity in JS
    const embRows = this.getChunkEmbeddings(bookHash)
    console.log('[Reedy][Db] loaded embeddings:', embRows.length)
    const chunksByHash = new Map<string, ChunkRow>()
    for (const c of this.getChunks(bookHash)) {
      if (spoilerBoundPosition !== undefined && c.position_index > spoilerBoundPosition) continue
      chunksByHash.set(c.id, c)
    }
    console.log('[Reedy][Db] loaded chunks:', chunksByHash.size)

    const vectorResults: Array<{ chunk: ChunkRow; distance: number }> = []
    for (const emb of embRows) {
      const chunk = chunksByHash.get(emb.chunk_id)
      if (!chunk) continue
      const sim = cosineSimilarity(embedding, emb.embedding)
      vectorResults.push({ chunk, distance: 1 - sim })
    }
    vectorResults.sort((a, b) => a.distance - b.distance)
    const vectorRows = vectorResults.slice(0, fetchK)
    console.log('[Reedy][Db] vector path:', vectorRows.length)

    // FTS5 path
    const ftsResults: Array<{ chunk: ChunkRow; rank: number }> = []
    if (queryText.trim().length > 0) {
      try {
        const spoilerFilter = spoilerBoundPosition !== undefined
          ? `AND reedy_book_chunks.position_index <= ${spoilerBoundPosition}`
          : ''
        const ftsQuery = this.db.prepare(`
          SELECT reedy_book_chunks.*, bm25(reedy_book_chunks_fts) AS metric
          FROM reedy_book_chunks_fts
          JOIN reedy_book_chunks ON reedy_book_chunks.rowid = reedy_book_chunks_fts.rowid
          WHERE reedy_book_chunks_fts MATCH ? AND reedy_book_chunks.book_hash = ? ${spoilerFilter}
          ORDER BY metric
          LIMIT ?
        `).all(queryText.replace(/[^\w\s\u4e00-\u9fff]/g, ' '), bookHash, fetchK) as Array<Record<string, unknown>>
        for (const row of ftsQuery) {
          ftsResults.push({
            chunk: {
              id: row.id as string,
              book_hash: row.book_hash as string,
              section_index: row.section_index as number,
              chapter_title: row.chapter_title as string | null,
              start_cfi: row.start_cfi as string,
              end_cfi: row.end_cfi as string,
              position_index: row.position_index as number,
              text: row.text as string,
              token_count: row.token_count as number
            },
            rank: row.metric as number
          })
        }
      } catch {
        // FTS5 may fail on empty index; vector path is primary
      }
    }

    const fused = reciprocalRankFusion(vectorRows, ftsResults, topK)
    console.log('[Reedy][Db] RRF results:', fused.length, fused.map(r => ({ id: r.chunk.id.slice(0, 30), score: r.score, vectorRank: r.vectorRank, ftsRank: r.ftsRank, text: r.chunk.text.slice(0, 60) })))
    return fused
  }

  // ---------------------------------------------------------------------------
  // memory
  // ---------------------------------------------------------------------------

  upsertMemory(args: {
    scope: MemoryScope
    scope_key: string
    key: string
    summary: string
    source_message_id?: string
    embedding?: Float32Array | number[]
  }): MemoryRow {
    const id = uuidv4()
    const now = Date.now()

    this.stmt.upsertMemory.run({
      id,
      scope: args.scope,
      scope_key: args.scope_key,
      key: args.key,
      summary: args.summary,
      source_message_id: args.source_message_id ?? null,
      updated_at: now
    })

    if (args.embedding) {
      const emb = args.embedding instanceof Float32Array ? args.embedding : new Float32Array(args.embedding as number[])
      this.stmt.insertMemoryEmbedding.run({
        memory_id: id,
        embedding: serializeEmbedding(emb)
      })
    }

    const row = this.stmt.getMemoryByKey.get(args.scope, args.scope_key, args.key) as { id?: string } | undefined
    if (!row?.id) throw new Error('Failed to save memory')
    return this.getMemory(row.id)
  }

  getMemory(id: string): MemoryRow {
    const row = this.db.prepare('SELECT * FROM reedy_memory WHERE id = ?').get(id) as Record<string, unknown>
    return {
      id: row.id as string,
      scope: row.scope as MemoryScope,
      scope_key: row.scope_key as string,
      key: row.key as string,
      summary: row.summary as string,
      source_message_id: row.source_message_id as string | null,
      updated_at: row.updated_at as number
    }
  }

  deleteMemory(id: string): boolean {
    this.stmt.deleteMemoryEmbedding.run(id)
    const result = this.stmt.deleteMemory.run(id)
    return result.changes > 0
  }

  listMemories(scope: MemoryScope, scopeKey: string, limit: number): MemoryRow[] {
    const rows = this.stmt.listMemories.all(scope, scopeKey, limit) as Record<string, unknown>[]
    return rows.map(r => ({
      id: r.id as string,
      scope: r.scope as MemoryScope,
      scope_key: r.scope_key as string,
      key: r.key as string,
      summary: r.summary as string,
      source_message_id: r.source_message_id as string | null,
      updated_at: r.updated_at as number
    }))
  }

  searchMemories(
    scope: MemoryScope,
    scopeKey: string,
    queryEmbedding: Float32Array | number[] | null,
    topK: number,
    recencyWeight: number = 0.1
  ): Array<MemoryRow & { score: number; vectorDistance: number | null }> {
    const limit = Math.max(1, topK)
    const now = Date.now()

    if (queryEmbedding && (queryEmbedding as Float32Array).length > 0) {
      const emb = queryEmbedding instanceof Float32Array ? queryEmbedding : new Float32Array(queryEmbedding as number[])
      const fetchK = Math.max(limit, limit * 3)

      const memRows = this.listMemories(scope, scopeKey, fetchK)
      const embMap = new Map<string, Float32Array>()

      const embIds = memRows.map(m => m.id)
      for (const id of embIds) {
        const row = this.db.prepare('SELECT embedding FROM reedy_memory_embeddings WHERE memory_id = ?').get(id) as { embedding: Buffer } | undefined
        if (row) embMap.set(id, deserializeEmbedding(row.embedding))
      }

      const maxAge = Math.max(1, ...memRows.map(r => Math.max(0, now - r.updated_at)))

      const scored = memRows.map(r => {
        const memEmb = embMap.get(r.id)
        const distance = memEmb ? (1 - cosineSimilarity(emb, memEmb)) : 1
        const ageNorm = (now - r.updated_at) / maxAge
        return {
          ...r,
          score: distance + recencyWeight * ageNorm,
          vectorDistance: distance
        }
      })

      return scored.sort((a, b) => a.score - b.score).slice(0, limit)
    }

    return this.listMemories(scope, scopeKey, limit).map(r => ({
      ...r,
      score: Math.max(0, (now - r.updated_at) / 1000),
      vectorDistance: null
    }))
  }

  // ---------------------------------------------------------------------------
  // skills
  // ---------------------------------------------------------------------------

  upsertSkill(skill: {
    id: string
    name: string
    description: string
    instructions: string
    tool_allowlist: string[] | null
    builtin: boolean
    enabled: boolean
  }): void {
    this.stmt.upsertSkill.run({
      id: skill.id,
      name: skill.name,
      description: skill.description,
      instructions: skill.instructions,
      tool_allowlist: skill.tool_allowlist ? JSON.stringify(skill.tool_allowlist) : null,
      builtin: skill.builtin ? 1 : 0,
      enabled: skill.enabled ? 1 : 0
    })
  }

  listSkills(): Array<{
    id: string
    name: string
    description: string
    instructions: string
    tool_allowlist: string[] | null
    builtin: boolean
    enabled: boolean
  }> {
    const rows = this.stmt.listSkills.all() as Record<string, unknown>[]
    return rows.map(r => ({
      id: r.id as string,
      name: r.name as string,
      description: r.description as string,
      instructions: r.instructions as string,
      tool_allowlist: r.tool_allowlist ? JSON.parse(r.tool_allowlist as string) : null,
      builtin: !!(r.builtin as number),
      enabled: !!(r.enabled as number)
    }))
  }

  getSkill(id: string) {
    const row = this.stmt.getSkill.get(id) as Record<string, unknown> | undefined
    if (!row) return null
    return {
      id: row.id as string,
      name: row.name as string,
      description: row.description as string,
      instructions: row.instructions as string,
      tool_allowlist: row.tool_allowlist ? JSON.parse(row.tool_allowlist as string) : null,
      builtin: !!(row.builtin as number),
      enabled: !!(row.enabled as number)
    }
  }

  setSkillEnabled(id: string, enabled: boolean): void {
    this.stmt.updateSkillEnabled.run({ id, enabled: enabled ? 1 : 0 })
  }

  // ---------------------------------------------------------------------------
  // metrics
  // ---------------------------------------------------------------------------

  insertMetric(event: {
    ts: number
    event: string
    book_hash?: string
    session_id?: string
    turn_id?: string
    message_id?: string
    app_version: string
    schema_version: number
    payload?: string
  }): void {
    this.stmt.insertMetric.run({
      ts: event.ts,
      event: event.event,
      book_hash: event.book_hash ?? null,
      session_id: event.session_id ?? null,
      turn_id: event.turn_id ?? null,
      message_id: event.message_id ?? null,
      app_version: event.app_version,
      schema_version: event.schema_version,
      payload: event.payload ?? null
    })
  }

  getMetrics(since: number = 0): Array<Record<string, unknown>> {
    return this.stmt.getMetrics.all(since) as Array<Record<string, unknown>>
  }
}

// ---------------------------------------------------------------------------
// RRF fusion
// ---------------------------------------------------------------------------

function reciprocalRankFusion(
  vectorResults: Array<{ chunk: ChunkRow; distance: number }>,
  ftsResults: Array<{ chunk: ChunkRow; rank: number }>,
  topK: number
): ScoredChunk[] {
  const merged = new Map<string, ScoredChunk>()

  for (let i = 0; i < vectorResults.length; i++) {
    const item = vectorResults[i]!
    const rank = i + 1
    merged.set(item.chunk.id, {
      chunk: item.chunk,
      score: 1 / (RRF_K + rank),
      vectorRank: rank,
      ftsRank: null
    })
  }

  for (let i = 0; i < ftsResults.length; i++) {
    const item = ftsResults[i]!
    const rank = i + 1
    const existing = merged.get(item.chunk.id)
    if (existing) {
      existing.score += 1 / (RRF_K + rank)
      existing.ftsRank = rank
    } else {
      merged.set(item.chunk.id, {
        chunk: item.chunk,
        score: 1 / (RRF_K + rank),
        vectorRank: null,
        ftsRank: rank
      })
    }
  }

  return [...merged.values()].sort((a, b) => b.score - a.score).slice(0, topK)
}
