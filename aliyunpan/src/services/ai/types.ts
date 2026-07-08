import type { LanguageModel, EmbeddingModel } from 'ai'

export type AIProviderName = 'ollama' | 'ai-gateway' | 'openrouter' | 'deepseek' | 'boxplayer-cloud'

export interface AISettings {
  provider: AIProviderName
  ollamaUrl: string
  ollamaModel: string
  ollamaEmbeddingModel: string
  aiGatewayApiKey: string
  aiGatewayModel: string
  aiGatewayEmbeddingModel: string
  openRouterApiKey: string
  openRouterBaseUrl: string
  openRouterModel: string
  openRouterEmbeddingModel: string
  spoilerProtection: boolean
  maxContextChunks: number
  indexingMode: 'on-demand' | 'background'
  reedy: { enabled: boolean; runtime: string }
}

export interface AIProvider {
  readonly id: AIProviderName
  readonly name: string
  readonly requiresAuth: boolean
  getModel(): LanguageModel
  getEmbeddingModel(): EmbeddingModel
  isAvailable(): Promise<boolean>
  healthCheck(): Promise<{ ok: boolean; error?: string }>
}

export interface TextChunk {
  id: string
  bookHash: string
  sectionIndex: number
  chapterTitle: string
  text: string
  embedding?: number[]
  pageNumber: number
}

export interface ScoredChunk extends TextChunk {
  score: number
  searchMethod: 'bm25' | 'vector' | 'hybrid'
}

export interface BookIndexMeta {
  bookHash: string
  totalSections: number
  totalChunks: number
  lastUpdated: number
}

export interface EmbeddingProgress {
  current: number
  total: number
  phase: 'chunking' | 'embedding' | 'indexing'
}

export interface AIConversation {
  id: string
  bookHash: string
  title: string
  mode: 'ask' | 'chat'
  createdAt: number
  updatedAt: number
}

export interface AIMessage {
  id: string
  conversationId: string
  role: 'user' | 'assistant'
  content: string
  createdAt: number
}
