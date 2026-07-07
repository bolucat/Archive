import { createGateway } from 'ai'
import type { LanguageModel, EmbeddingModel } from 'ai'
import type { AISettings, AIProvider } from '../types'
import { GATEWAY_MODELS } from '../constants'
import { AI_TIMEOUTS } from '../utils/retry'
import { aiLogger } from '../logger'

export class AIGatewayProvider implements AIProvider {
  readonly id = 'ai-gateway' as const
  readonly name = 'AI Gateway (Cloud)'
  readonly requiresAuth = true
  private gateway: ReturnType<typeof createGateway>
  private apiKey: string
  private modelId: string
  private embedModel: string

  constructor(settings: AISettings) {
    this.apiKey = settings.aiGatewayApiKey
    this.modelId = settings.aiGatewayModel || GATEWAY_MODELS.GEMINI_FLASH_LITE
    this.embedModel = settings.aiGatewayEmbeddingModel || 'openai/text-embedding-3-small'
    this.gateway = createGateway({ apiKey: this.apiKey })
    aiLogger.provider.start('AIGatewayProvider initialized')
  }

  getModel(): LanguageModel {
    return this.gateway(this.modelId)
  }

  getEmbeddingModel(): EmbeddingModel {
    return this.gateway.embedding(this.embedModel)
  }

  async isAvailable(): Promise<boolean> {
    return !!this.apiKey
  }

  async healthCheck(): Promise<{ ok: boolean; error?: string }> {
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
      aiLogger.provider.error('AIGateway health check failed', e.message)
      return { ok: false, error: e?.message || 'health check failed' }
    }
  }
}
