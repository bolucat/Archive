import type { AISettings, ScoredChunk, EmbeddingProgress } from '../types'
import type { RetrievalBackend } from './retrievalBackend'
import { reedyClient } from '../../reedy/ReedyClient'
import { getAIProvider } from '../providers'
import type { EmbeddingModel } from 'ai'
import { canUseSemanticEmbeddings } from '../embeddingPolicy'

export class ReedyBackend implements RetrievalBackend {
  readonly kind = 'reedy' as const

  constructor(private readonly settings: AISettings) {}

  async isIndexed(bookHash: string): Promise<boolean> {
    return reedyClient.isIndexed(bookHash)
  }

  private getEmbeddingModelName(): string {
    const s = this.settings
    if (!canUseSemanticEmbeddings(s.provider)) return 'local-keyword'
    switch (s.provider) {
      case 'ollama':
        return s.ollamaEmbeddingModel || 'nomic-embed-text'
      case 'ai-gateway':
        return s.aiGatewayEmbeddingModel || 'openai/text-embedding-3-small'
      case 'openrouter':
        return s.openRouterEmbeddingModel || 'openai/text-embedding-3-small'
      default:
        return 'unknown'
    }
  }

  private getBatchSize(): number {
    return this.settings.provider === 'ollama' ? 4 : 16
  }

  async indexBook(
    bookHash: string,
    sections: Array<{ index: number; title: string; text: string; doc?: Document }>,
    settings: AISettings,
    options?: {
      onProgress?: (progress: EmbeddingProgress) => void
      signal?: AbortSignal
    }
  ): Promise<void> {
    const embeddingModel = canUseSemanticEmbeddings(settings.provider)
      ? getAIProvider(settings).getEmbeddingModel?.() as EmbeddingModel | undefined
      : undefined

    // Chunking phase (still need to run in renderer for DOM access)
    options?.onProgress?.({ phase: 'chunking', current: 0, total: sections.length })

    const { chunkSection, chunkPlainText } = await import('../../reedy/CfiChunker')
    let allChunks: Array<{
      id: string
      book_hash: string
      section_index: number
      chapter_title: string | null
      start_cfi: string
      end_cfi: string
      position_index: number
      text: string
      token_count: number
    }> = []

    let globalPosition = 0
    for (let i = 0; i < sections.length; i++) {
      if (options?.signal?.aborted) throw new Error('Indexing aborted')
      const section = sections[i]!
      let chunks: any[]
      if (section.doc) {
        chunks = chunkSection(section.doc as any, section.index, section.title, bookHash)
      } else if (section.text) {
        chunks = chunkPlainText(section.text, section.index, section.title, bookHash)
      } else {
        continue
      }
      for (const c of chunks) {
        allChunks.push({
          ...c,
          position_index: globalPosition++
        } as any)
      }
      options?.onProgress?.({ phase: 'chunking', current: i + 1, total: sections.length })
    }

    const modelName = this.getEmbeddingModelName()

    if (allChunks.length === 0) {
      await reedyClient.storeMeta({
        book_hash: bookHash,
        indexing_status: 'empty_index',
        chunk_count: 0,
        embedding_model: modelName,
        embedding_dim: 0,
        indexed_at: null,
        error: 'No extractable text found'
      })
      return
    }

    // Update meta: indexing
    await reedyClient.storeMeta({
      book_hash: bookHash,
      indexing_status: 'indexing',
      chunk_count: allChunks.length,
      embedding_model: modelName,
      embedding_dim: 0,
      indexed_at: null,
      error: null
    })

    // Store chunks
    await reedyClient.storeChunks(allChunks)

    let dim = 0
    if (embeddingModel) {
      try {
      const { embedMany } = await import('ai')
      const texts = allChunks.map((c) => c.text)
      const batchSize = this.getBatchSize()
      options?.onProgress?.({ phase: 'embedding', current: 0, total: texts.length })

      for (let i = 0; i < texts.length; i += batchSize) {
        if (options?.signal?.aborted) throw new Error('Indexing aborted')
        const batch = texts.slice(i, i + batchSize)
        const result = await embedMany({ model: embeddingModel, values: batch })
        const embs = result.embeddings
        if (embs.length > 0 && dim === 0) dim = embs[0]!.length
        const embRows = embs.map((emb, idx) => ({
          chunk_id: allChunks[i + idx]!.id,
          book_hash: bookHash,
          embedding: new Float32Array(emb)
        }))
        await reedyClient.storeEmbeddings(embRows)
        options?.onProgress?.({ phase: 'embedding', current: Math.min(i + batchSize, texts.length), total: texts.length })
      }
      } catch (embErr: any) {
        console.warn('[ReedyBackend] embedding failed, using FTS-only index:', embErr?.message || embErr)
        dim = 0
      }
    } else {
      options?.onProgress?.({ phase: 'embedding', current: allChunks.length, total: allChunks.length })
    }

    // Finalize meta
    await reedyClient.storeMeta({
      book_hash: bookHash,
      indexing_status: 'indexed',
      chunk_count: allChunks.length,
      embedding_model: modelName,
      embedding_dim: dim,
      indexed_at: Date.now(),
      error: null
    })
  }

  async clearBook(bookHash: string): Promise<void> {
    await reedyClient.clearBook(bookHash)
  }

  async searchForSystemPrompt(query: string, bookHash: string, settings: AISettings, topK: number = 10, maxPage?: number): Promise<ScoredChunk[]> {
    const embeddingModel = canUseSemanticEmbeddings(settings.provider)
      ? getAIProvider(settings).getEmbeddingModel?.() as EmbeddingModel | undefined
      : undefined
    if (!embeddingModel) {
      const chunks = await reedyClient.search(bookHash, new Float32Array(0), query, topK, maxPage)
      return chunks.map((c) => this.toAIScoredChunk(c))
    }

    try {
      const { embed } = await import('ai')
      const result = await embed({ model: embeddingModel, value: query })
      if (!result.embedding || result.embedding.length === 0) {
        const chunks = await reedyClient.search(bookHash, new Float32Array(0), query, topK, maxPage)
        return chunks.map((c) => this.toAIScoredChunk(c))
      }
      const qEmb = new Float32Array(result.embedding)
      const chunks = await reedyClient.search(bookHash, qEmb, query, topK, maxPage)
      return chunks.map((c) => this.toAIScoredChunk(c))
    } catch {
      const chunks = await reedyClient.search(bookHash, new Float32Array(0), query, topK, maxPage)
      return chunks.map((c) => this.toAIScoredChunk(c))
    }
  }

  private toAIScoredChunk(c: any): ScoredChunk {
    return {
      searchMethod: c.vectorRank && c.ftsRank ? 'hybrid' : c.vectorRank ? 'vector' : 'bm25',
      id: c.chunk?.id || '',
      bookHash: c.chunk?.book_hash || '',
      sectionIndex: c.chunk?.section_index || 0,
      chapterTitle: c.chunk?.chapter_title || '',
      text: c.chunk?.text || '',
      pageNumber: c.chunk?.position_index || 0,
      score: c.score || 0,
      embedding: []
    }
  }
}
