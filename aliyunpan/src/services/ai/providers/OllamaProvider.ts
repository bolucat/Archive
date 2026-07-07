import { createOpenAI } from '@ai-sdk/openai'
import type { LanguageModel, EmbeddingModel } from 'ai'
import type { AISettings, AIProvider } from '../types'
import { AI_TIMEOUTS } from '../utils/retry'
import { aiLogger } from '../logger'

export class OllamaProvider implements AIProvider {
  readonly id = 'ollama' as const
  readonly name = 'Ollama (Local)'
  readonly requiresAuth = false
  private client: ReturnType<typeof createOpenAI>
  private ollamaModel: string
  private ollamaEmbeddingModel: string
  private baseUrl: string

  constructor(settings: AISettings) {
    this.ollamaModel = settings.ollamaModel || 'llama3.2'
    this.ollamaEmbeddingModel = settings.ollamaEmbeddingModel || 'nomic-embed-text'
    this.baseUrl = settings.ollamaUrl || 'http://127.0.0.1:11434'
    this.client = createOpenAI({
      name: 'ollama',
      baseURL: `${this.baseUrl}/v1`,
      apiKey: 'ollama',
    })
    aiLogger.provider.start('OllamaProvider initialized', this.baseUrl)
  }

  getModel(): LanguageModel {
    return this.client.chat(this.ollamaModel)
  }

  getEmbeddingModel(): EmbeddingModel {
    return this.client.embedding(this.ollamaEmbeddingModel)
  }

  async isAvailable(): Promise<boolean> {
    try {
      const controller = new AbortController()
      const timer = setTimeout(() => controller.abort(), AI_TIMEOUTS.OLLAMA_CONNECT)
      const resp = await fetch(`${this.baseUrl}/api/tags`, { signal: controller.signal })
      clearTimeout(timer)
      return resp.ok
    } catch {
      return false
    }
  }

  async healthCheck(): Promise<{ ok: boolean; error?: string }> {
    const available = await this.isAvailable()
    if (!available) return { ok: false, error: 'Ollama 服务未启动或无法连接' }
    try {
      const { generateText } = await import('ai')
      const result = await generateText({
        model: this.getModel(),
        prompt: 'Reply with exactly: OK',
        maxOutputTokens: 5,
        abortSignal: AbortSignal.timeout(AI_TIMEOUTS.HEALTH_CHECK),
      })
      return { ok: !!(result.text && result.text.trim().length > 0) }
    } catch (e: any) {
      aiLogger.provider.error('Ollama health check failed', e.message)
      return { ok: false, error: e?.message || 'health check failed' }
    }
  }
}
