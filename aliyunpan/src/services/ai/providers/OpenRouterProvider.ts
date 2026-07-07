import { createOpenAICompatible } from '@ai-sdk/openai-compatible'
import type { LanguageModel, EmbeddingModel } from 'ai'
import type { AISettings, AIProvider } from '../types'
import { AI_TIMEOUTS } from '../utils/retry'
import { aiLogger } from '../logger'

export interface OpenRouterModelInfo {
  id: string
  name?: string
  created?: number
  description?: string
  context_length?: number
  pricing?: { prompt: string; completion: string }
}

export async function fetchOpenRouterModels(
  baseUrl: string,
  apiKey: string,
  signal?: AbortSignal
): Promise<OpenRouterModelInfo[]> {
  const resp = await fetch(`${baseUrl.replace(/\/+$/, '')}/models`, {
    headers: { Authorization: `Bearer ${apiKey}` },
    signal,
  })
  if (!resp.ok) throw new Error(`Failed to fetch models: ${resp.status}`)
  const json = await resp.json()
  return json.data || []
}

export class OpenRouterProvider implements AIProvider {
  readonly id = 'openrouter' as const
  readonly name = 'OpenRouter (Custom)'
  readonly requiresAuth = true
  private client: ReturnType<typeof createOpenAICompatible>
  private apiKey: string
  private modelId: string
  private embedModel: string
  private baseUrl: string

  constructor(settings: AISettings) {
    this.apiKey = settings.openRouterApiKey
    this.modelId = settings.openRouterModel || 'openai/gpt-4o-mini'
    this.embedModel = settings.openRouterEmbeddingModel || 'openai/text-embedding-3-small'
    this.baseUrl = settings.openRouterBaseUrl || 'https://openrouter.ai/api/v1'
    this.client = createOpenAICompatible({
      name: settings.provider || 'openai-compatible',
      baseURL: this.baseUrl,
      apiKey: this.apiKey,
      headers: {
        'HTTP-Referer': 'https://boxplayer.app',
        'X-Title': 'BoxPlayer',
      },
    })
    aiLogger.provider.start('OpenRouterProvider initialized', this.baseUrl)
  }

  getModel(): LanguageModel {
    return this.client.chatModel(this.modelId)
  }

  getEmbeddingModel(): EmbeddingModel {
    return this.client.textEmbeddingModel(this.embedModel)
  }

  async isAvailable(): Promise<boolean> {
    return !!this.apiKey
  }

  async healthCheck(): Promise<{ ok: boolean; error?: string }> {
    try {
      const resp = await fetch(`${this.baseUrl.replace(/\/+$/, '')}/chat/completions`, {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${this.apiKey}`,
          'Content-Type': 'application/json',
          'HTTP-Referer': 'https://boxplayer.app',
          'X-Title': 'BoxPlayer',
        },
        body: JSON.stringify({ model: this.modelId, max_tokens: 50, messages: [{ role: 'user', content: 'Reply with exactly: OK' }] }),
        signal: AbortSignal.timeout(AI_TIMEOUTS.HEALTH_CHECK),
      })
      if (!resp.ok) {
        const text = await resp.text().catch(() => resp.statusText)
        return { ok: false, error: `HTTP ${resp.status}: ${text.slice(0, 200)}` }
      }
      const data = await resp.json()
      // OpenAI-compatible format
      let msg = data?.choices?.[0]?.message || {}
      let text = msg.content || ''
      // Reasoning models put output in reasoning_content
      if (!text && typeof msg.reasoning_content === 'string') text = msg.reasoning_content
      // Some older APIs return text directly
      if (!text && typeof data?.choices?.[0]?.text === 'string') text = data.choices[0].text
      // Log raw for debugging
      if (!text) console.log('[OpenRouter] health check response:', JSON.stringify(data).slice(0, 300))
      return { ok: text.trim().length > 0, error: text.trim().length > 0 ? undefined : `模型返回空内容 (model: ${this.modelId})` }
    } catch (e: any) {
      return { ok: false, error: e?.message || 'health check failed' }
    }
  }
}
