import type { BookMeta, ScoredChunk, MemoryScope, MemoryRow } from './types'

function invoke<T = any>(channel: string, ...args: any[]): Promise<T> {
  const fn = (typeof window !== 'undefined' ? (window as any).ReedyInvoke : null) as ((ch: string, ...a: any[]) => Promise<T>) | null
  if (!fn) {
    console.error('[Reedy][Client] ReedyInvoke not available on window')
    throw new Error('ReedyClient: ReedyInvoke not available — ensure preload script is loaded')
  }
  console.log('[Reedy][Client] IPC invoke:', channel, args.length ? args.map(a => typeof a === 'object' ? `[object ${a?.length !== undefined ? 'array/' + a.length : 'object'}]` : a) : [])
  return fn(channel, ...args).then((res: T) => {
    console.log('[Reedy][Client] IPC response:', channel, typeof res === 'object' ? (Array.isArray(res) ? `array(${res.length})` : 'object') : typeof res)
    return res
  }).catch((err: any) => {
    console.error('[Reedy][Client] IPC error:', channel, err?.message || err)
    throw err
  })
}

export class ReedyClient {
  async getMeta(bookHash: string): Promise<BookMeta | null> {
    return invoke('reedy:get-meta', bookHash)
  }

  async isIndexed(bookHash: string): Promise<boolean> {
    return invoke('reedy:is-indexed', bookHash)
  }

  async clearBook(bookHash: string): Promise<void> {
    await invoke('reedy:clear-book', bookHash)
  }

  async storeChunks(chunks: Array<{
    id: string; book_hash: string; section_index: number
    chapter_title: string | null; start_cfi: string; end_cfi: string
    position_index: number; text: string; token_count: number
  }>): Promise<void> {
    await invoke('reedy:store-chunks', chunks)
  }

  async storeEmbeddings(rows: Array<{ chunk_id: string; book_hash: string; embedding: Float32Array }>): Promise<void> {
    const serialized = rows.map(r => ({
      chunk_id: r.chunk_id,
      book_hash: r.book_hash,
      embedding: Array.from(r.embedding)
    }))
    await invoke('reedy:store-embeddings', serialized)
  }

  async storeMeta(meta: BookMeta): Promise<void> {
    await invoke('reedy:store-meta', meta)
  }

  async search(
    bookHash: string,
    queryEmbedding: Float32Array | number[],
    queryText: string,
    topK: number,
    spoilerBound?: number
  ): Promise<ScoredChunk[]> {
    const emb = Array.from(queryEmbedding instanceof Float32Array ? queryEmbedding : new Float32Array(queryEmbedding as number[]))
    return invoke('reedy:search', bookHash, emb, queryText, topK, spoilerBound)
  }

  async writeMemory(args: {
    scope: MemoryScope; scope_key: string; key: string; summary: string
    source_message_id?: string; embedding?: Float32Array | number[]
  }): Promise<MemoryRow> {
    const serialized = { ...args }
    if (serialized.embedding) {
      serialized.embedding = Array.from(
        serialized.embedding instanceof Float32Array ? serialized.embedding : new Float32Array(serialized.embedding as number[])
      )
    }
    return invoke('reedy:write-memory', serialized)
  }

  async searchMemories(
    scope: MemoryScope, scopeKey: string,
    queryEmbedding: Float32Array | number[] | null, topK: number, recencyWeight?: number
  ): Promise<Array<MemoryRow & { score: number; vectorDistance: number | null }>> {
    const emb = queryEmbedding ? Array.from(
      queryEmbedding instanceof Float32Array ? queryEmbedding : new Float32Array(queryEmbedding as number[])
    ) : null
    return invoke('reedy:search-memories', scope, scopeKey, emb, topK, recencyWeight)
  }

  async listMemories(scope: MemoryScope, scopeKey: string, limit: number): Promise<MemoryRow[]> {
    return invoke('reedy:list-memories', scope, scopeKey, limit)
  }

  async deleteMemory(id: string): Promise<boolean> {
    return invoke('reedy:delete-memory', id)
  }

  async listSkills() {
    return invoke('reedy:list-skills')
  }

  async toggleSkill(skillId: string, enabled: boolean): Promise<void> {
    await invoke('reedy:toggle-skill', skillId, enabled)
  }

  async exportMetrics(since?: number) {
    return invoke('reedy:export-metrics', since)
  }

  async recordMetric(evt: any): Promise<void> {
    await invoke('reedy:record-metric', evt)
  }

  async wipeAllData(): Promise<void> {
    await invoke('reedy:wipe-all')
  }

  async destroy(): Promise<void> {
    await invoke('reedy:destroy')
  }
}

export const reedyClient = new ReedyClient()
