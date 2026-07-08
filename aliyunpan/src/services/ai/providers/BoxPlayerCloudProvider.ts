import { createOpenAICompatible } from '@ai-sdk/openai-compatible'
import type { LanguageModel, EmbeddingModel } from 'ai'
import Config from '../../../config'
import type { AISettings, AIProvider } from '../types'
import { getBoxPlayerAccessToken } from '../../../utils/boxplayerAuth'
import { completeBoxPlayerCloudChat } from '../../../utils/boxplayerCloudAI'
import { aiLogger } from '../logger'

const CLOUD_AI_BASE_URL = ((Config as any).BOXPLAYER_AI_API_URL || 'https://ai.xbyvideohub.com').replace(/\/+$/, '')

function getCloudAIBaseURL(): string {
  return CLOUD_AI_BASE_URL
}

function createBoxPlayerCloudModel(modelId: string) {
  const provider = createOpenAICompatible({
    name: 'boxplayer-cloud',
    baseURL: `${getCloudAIBaseURL()}/v1`,
    fetch: async (url: any, init?: any) => {
      const token = await getBoxPlayerAccessToken()
      let body = init?.body
      if (typeof body === 'string') {
        try { body = JSON.stringify({ ...JSON.parse(body), feature: 'ai_search' }) } catch {}
      }
      const rewrittenUrl = String(url).replace('/chat/completions', '/chat/completions')
      return globalThis.fetch(rewrittenUrl, {
        ...init,
        headers: { ...(init?.headers || {}), Authorization: `Bearer ${token}` },
        body,
      })
    },
  })
  return provider.chatModel(modelId)
}

export class BoxPlayerCloudProvider implements AIProvider {
  readonly id = 'boxplayer-cloud' as const
  readonly name = 'BoxPlayer Cloud AI'
  readonly requiresAuth = true
  private modelId: string

  constructor(settings: AISettings) {
    this.modelId = settings.openRouterModel || 'deepseek/deepseek-v4-pro'
    aiLogger.provider.start('BoxPlayerCloudProvider initialized', getCloudAIBaseURL())
  }

  getModel(): LanguageModel {
    return createBoxPlayerCloudModel(this.modelId)
  }

  getEmbeddingModel(): EmbeddingModel {
    const self = this
    return {
      specificationVersion: 'v3' as const,
      provider: 'boxplayer-cloud',
      modelId: '@cf/baai/bge-m3',
      maxEmbeddingsPerCall: 64,
      supportsParallelCalls: false,
      async doEmbed({ values, abortSignal }: { values: string[]; abortSignal?: AbortSignal }) {
        const token = await getBoxPlayerAccessToken()
        const resp = await fetch(`${getCloudAIBaseURL()}/v1/embeddings`, {
          method: 'POST',
          headers: {
            Authorization: `Bearer ${token}`,
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({ text: values }),
          signal: abortSignal,
        })
        if (!resp.ok) {
          const data = await resp.json().catch(() => ({ error: `HTTP ${resp.status}` }))
          throw new Error(data.error || `HTTP ${resp.status}`)
        }
        const data = await resp.json() as { result?: { data?: number[][]; shape?: number[] } }
        const result = data.result
        const embeddings = result?.data
        if (!embeddings) throw new Error('Embedding response missing data')
        return {
          embeddings,
          usage: { tokens: 0 },
          warnings: [],
        }
      },
    } as EmbeddingModel
  }

  async isAvailable(): Promise<boolean> {
    try {
      const token = await getBoxPlayerAccessToken()
      return !!token
    } catch {
      return false
    }
  }

  async healthCheck(): Promise<{ ok: boolean; error?: string }> {
    try {
      await completeBoxPlayerCloudChat({
        feature: 'reader_chat',
        messages: [{ role: 'user', content: '请只回复 OK' }],
      })
      return { ok: true }
    } catch (e: any) {
      return { ok: false, error: e?.message || '连接失败' }
    }
  }
}
