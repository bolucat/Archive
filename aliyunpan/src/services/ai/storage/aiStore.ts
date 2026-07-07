import DB from '../../../utils/db'
import type { TextChunk, ScoredChunk, BookIndexMeta, AIConversation, AIMessage } from '../types'
import { aiLogger } from '../logger'

function normalizeText(input: string): string {
  return input.replace(/\s+/g, ' ').trim()
}

function cosineSimilarity(a: number[], b: number[]): number {
  const len = Math.min(a.length, b.length)
  let dot = 0; let normA = 0; let normB = 0
  for (let i = 0; i < len; i++) {
    dot += a[i] * b[i]; normA += a[i] * a[i]; normB += b[i] * b[i]
  }
  if (!normA || !normB) return 0
  return dot / (Math.sqrt(normA) * Math.sqrt(normB))
}

function keywordScore(text: string, terms: string[]): number {
  const lower = text.toLowerCase()
  return terms.reduce((sum, term) => sum + (lower.split(term).length - 1), 0)
}

const META_KEY = (hash: string) => `ai_book_meta_${hash}`
const CHUNKS_KEY = (hash: string) => `ai_book_chunks_${hash}`
const CONV_PREFIX = 'ai_conversation_'
const MSG_PREFIX = 'ai_message_'

class AIStore {
  // --- Meta ---
  async saveMeta(meta: BookIndexMeta) {
    await DB.saveValueObject(META_KEY(meta.bookHash), meta)
    aiLogger.store.complete('saveMeta', meta.bookHash)
  }

  async getMeta(bookHash: string): Promise<BookIndexMeta | null> {
    return (await DB.getValueObject(META_KEY(bookHash)) as BookIndexMeta) || null
  }

  async isIndexed(bookHash: string): Promise<boolean> {
    return !!(await this.getMeta(bookHash))
  }

  // --- Chunks ---
  async saveChunks(chunks: TextChunk[]) {
    if (!chunks.length) return
    const bookHash = chunks[0].bookHash
    const existing = await this.getChunks(bookHash)
    const map = new Map(existing.map((c) => [c.id, c]))
    for (const c of chunks) map.set(c.id, c)
    await DB.saveValueObject(CHUNKS_KEY(bookHash), Array.from(map.values()))
    aiLogger.store.complete('saveChunks', `${chunks.length} chunks for ${bookHash}`)
  }

  async getChunks(bookHash: string): Promise<TextChunk[]> {
    const value = await DB.getValueObject(CHUNKS_KEY(bookHash))
    return Array.isArray(value) ? value : []
  }

  // --- Search ---
  async vectorSearch(bookHash: string, queryEmbedding: number[], topK: number, maxPage?: number): Promise<ScoredChunk[]> {
    const chunks = await this.getChunks(bookHash)
    let scored = chunks
      .filter((c) => c.embedding && (maxPage == null || c.pageNumber <= maxPage))
      .map((c) => ({ ...c, score: cosineSimilarity(queryEmbedding, c.embedding!), searchMethod: 'vector' as const }))
    scored.sort((a, b) => b.score - a.score)
    return scored.slice(0, topK)
  }

  async bm25Search(bookHash: string, query: string, topK: number, maxPage?: number): Promise<ScoredChunk[]> {
    const chunks = await this.getChunks(bookHash)
    const terms = normalizeText(query).toLowerCase().split(/\s+/).filter(Boolean)
    let scored = chunks
      .filter((c) => maxPage == null || c.pageNumber <= maxPage)
      .map((c) => ({ ...c, score: keywordScore(c.text, terms), searchMethod: 'bm25' as const }))
    scored.sort((a, b) => b.score - a.score)
    return scored.slice(0, topK)
  }

  async hybridSearch(bookHash: string, queryEmbedding: number[] | null, query: string, topK: number, maxPage?: number): Promise<ScoredChunk[]> {
    const chunks = await this.getChunks(bookHash)
    const terms = normalizeText(query).toLowerCase().split(/\s+/).filter(Boolean)

    let scored = chunks
      .filter((c) => maxPage == null || c.pageNumber <= maxPage)
      .map((c) => {
        const vectorScore = queryEmbedding && c.embedding ? cosineSimilarity(queryEmbedding, c.embedding) * 1.0 : 0
        const bm25Score = keywordScore(c.text, terms) * 0.8
        return { ...c, score: vectorScore + bm25Score, searchMethod: (queryEmbedding ? 'hybrid' : 'bm25') as ScoredChunk['searchMethod'] }
      })

    // Deduplicate by text prefix
    const seen = new Set<string>()
    const deduped: typeof scored = []
    for (const c of scored.sort((a, b) => b.score - a.score)) {
      const key = c.text.slice(0, 100)
      if (!seen.has(key)) { seen.add(key); deduped.push(c) }
    }

    aiLogger.search.complete('hybridSearch', `${deduped.length} results / ${topK} topK`)
    return deduped.slice(0, topK)
  }

  // --- Clear ---
  async clearBook(bookHash: string) {
    await DB.deleteValueObject(META_KEY(bookHash))
    await DB.deleteValueObject(CHUNKS_KEY(bookHash))
    aiLogger.store.complete('clear', bookHash)
  }

  // --- Conversations ---
  async saveConversation(conv: AIConversation) {
    await DB.saveValueObject(`${CONV_PREFIX}${conv.id}`, conv)
  }

  async getConversations(bookHash: string): Promise<AIConversation[]> {
    // Get all conversations and filter by bookHash
    const all = await DB.getValueObject(`${CONV_PREFIX}index`)
    const ids: string[] = Array.isArray(all) ? all : []
    const results: AIConversation[] = []
    for (const id of ids) {
      const conv = await DB.getValueObject(`${CONV_PREFIX}${id}`) as AIConversation | null
      if (conv && conv.bookHash === bookHash) results.push(conv)
    }
    return results.sort((a, b) => b.updatedAt - a.updatedAt)
  }

  async deleteConversation(id: string) {
    await DB.deleteValueObject(`${CONV_PREFIX}${id}`)
    const all = await DB.getValueObject(`${CONV_PREFIX}index`)
    const ids: string[] = Array.isArray(all) ? all.filter((i) => i !== id) : []
    await DB.saveValueObject(`${CONV_PREFIX}index`, ids)
  }

  async updateConversationTitle(id: string, title: string) {
    const conv = await DB.getValueObject(`${CONV_PREFIX}${id}`) as AIConversation | null
    if (conv) {
      conv.title = title
      conv.updatedAt = Date.now()
      await this.saveConversation(conv)
    }
  }

  async ensureConversationIndex(id: string) {
    const all = await DB.getValueObject(`${CONV_PREFIX}index`)
    const ids: string[] = Array.isArray(all) ? all : []
    if (!ids.includes(id)) {
      ids.unshift(id)
      await DB.saveValueObject(`${CONV_PREFIX}index`, ids)
    }
  }

  // --- Messages ---
  async saveMessage(msg: AIMessage) {
    await DB.saveValueObject(`${MSG_PREFIX}${msg.id}`, msg)
    await this.ensureConversationIndex(msg.conversationId)
  }

  async getMessages(conversationId: string): Promise<AIMessage[]> {
    const all = await DB.getValueObject(`${MSG_PREFIX}index_${conversationId}`)
    const ids: string[] = Array.isArray(all) ? all : []
    const results: AIMessage[] = []
    for (const id of ids) {
      const msg = await DB.getValueObject(`${MSG_PREFIX}${id}`) as AIMessage | null
      if (msg && msg.conversationId === conversationId) results.push(msg)
    }
    return results.sort((a, b) => a.createdAt - b.createdAt)
  }

  async saveMessagesBatch(messages: AIMessage[]) {
    for (const msg of messages) {
      await DB.saveValueObject(`${MSG_PREFIX}${msg.id}`, msg)
    }
    if (messages.length) {
      const conversationId = messages[0].conversationId
      await this.ensureConversationIndex(conversationId)
      const all = await DB.getValueObject(`${MSG_PREFIX}index_${conversationId}`)
      const ids: string[] = Array.isArray(all) ? all : []
      for (const msg of messages) {
        if (!ids.includes(msg.id)) ids.push(msg.id)
      }
      await DB.saveValueObject(`${MSG_PREFIX}index_${conversationId}`, ids)
    }
  }

  async deleteMessages(conversationId: string) {
    const all = await DB.getValueObject(`${MSG_PREFIX}index_${conversationId}`)
    const ids: string[] = Array.isArray(all) ? all : []
    for (const id of ids) await DB.deleteValueObject(`${MSG_PREFIX}${id}`)
    await DB.deleteValueObject(`${MSG_PREFIX}index_${conversationId}`)
  }

  // --- Recovery ---
  async recoverFromError() {
    aiLogger.store.error('AIStore recovery triggered')
  }
}

export const aiStore = new AIStore()
