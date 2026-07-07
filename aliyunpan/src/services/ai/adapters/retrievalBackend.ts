import type { AISettings, TextChunk, ScoredChunk, EmbeddingProgress } from '../types'

export type RetrievalBackendKind = 'legacy-idb' | 'reedy'

export interface BackendIndexOptions {
  onProgress?: (progress: EmbeddingProgress) => void
  signal?: AbortSignal
}

export interface RetrievalBackend {
  readonly kind: RetrievalBackendKind
  isIndexed(bookHash: string): Promise<boolean>
  indexBook(bookHash: string, sections: Array<{ index: number; title: string; text: string }>, settings: AISettings, options?: BackendIndexOptions): Promise<void>
  clearBook(bookHash: string): Promise<void>
  searchForSystemPrompt(query: string, bookHash: string, settings: AISettings, topK?: number, maxPage?: number): Promise<ScoredChunk[]>
}

import { aiStore } from '../storage/aiStore'
import { chunkSection } from '../utils/chunker'
import { getAIProvider } from '../providers'
import { withRetryAndTimeout, AI_TIMEOUTS, AI_RETRY_CONFIGS } from '../utils/retry'
import { aiLogger } from '../logger'
import { ReedyBackend } from './ReedyBackend'

export class LegacyIdbBackend implements RetrievalBackend {
  readonly kind = 'legacy-idb' as const
  private settings: AISettings

  constructor(settings: AISettings) {
    this.settings = settings
  }

  async isIndexed(bookHash: string): Promise<boolean> {
    return aiStore.isIndexed(bookHash)
  }

  async indexBook(
    bookHash: string,
    sections: Array<{ index: number; title: string; text: string }>,
    settings: AISettings,
    options?: BackendIndexOptions
  ): Promise<void> {
    if (await this.isIndexed(bookHash)) return

    // Chunk
    options?.onProgress?.({ current: 0, total: sections.length || 1, phase: 'chunking' })
    aiLogger.chunker.start('indexBook', sections.length)
    let cumulative = 0
    const allChunks: TextChunk[] = []
    for (const section of sections) {
      const chunks = chunkSection(section.text, section.index, section.title, bookHash, cumulative)
      allChunks.push(...chunks)
      cumulative += section.text.length
    }
    aiLogger.chunker.complete('chunked', allChunks.length)

    // Embed
    if (allChunks.length) {
      options?.onProgress?.({ current: 0, total: allChunks.length, phase: 'embedding' })
      aiLogger.embedding.start('embedMany', allChunks.length)
      try {
        const provider = getAIProvider(settings)
        const { embedMany } = await import('ai')
        const { embeddings } = await withRetryAndTimeout(
          () => embedMany({ model: provider.getEmbeddingModel(), values: allChunks.map((c) => c.text) }),
          AI_TIMEOUTS.EMBEDDING_BATCH,
          AI_RETRY_CONFIGS.EMBEDDING
        )
        allChunks.forEach((c, i) => { c.embedding = embeddings[i] })
        aiLogger.embedding.complete('embedded', allChunks.length)
      } catch (e) {
        aiLogger.embedding.error('embedding failed', e)
      }
    }

    // Save
    options?.onProgress?.({ current: 0, total: 2, phase: 'indexing' })
    await aiStore.saveChunks(allChunks)
    await aiStore.saveMeta({ bookHash, totalSections: sections.length, totalChunks: allChunks.length, lastUpdated: Date.now() })
    aiLogger.rag.complete('indexed', bookHash)
  }

  async clearBook(bookHash: string): Promise<void> {
    await aiStore.clearBook(bookHash)
  }

  async searchForSystemPrompt(query: string, bookHash: string, settings: AISettings, topK = 10, maxPage?: number): Promise<ScoredChunk[]> {
    const provider = getAIProvider(settings)
    let queryEmbedding: number[] | null = null
    try {
      const { embed } = await import('ai')
      const result = await withRetryAndTimeout(
        () => embed({ model: provider.getEmbeddingModel(), value: query }),
        AI_TIMEOUTS.EMBEDDING_SINGLE,
        AI_RETRY_CONFIGS.EMBEDDING
      )
      queryEmbedding = result.embedding
    } catch {
      aiLogger.embedding.error('query embedding failed, falling back to BM25')
    }
    return aiStore.hybridSearch(bookHash, queryEmbedding, query, topK, maxPage)
  }
}

export function selectBackend(settings: AISettings): RetrievalBackend {
  if (settings.reedy?.enabled) {
    return new ReedyBackend(settings)
  }
  return new LegacyIdbBackend(settings)
}
