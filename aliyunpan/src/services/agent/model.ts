import type { Model } from '@earendil-works/pi-ai'
import Config from '../../config'
import { getBoxPlayerAccessToken } from '../../utils/boxplayerAuth'
import type { AgentSurface, BoxPlayerAgentModelConfig } from './types'
import { resolveAgentModelEndpoint } from '@shared/agentModelTransport'

// These limits intentionally match boxplayer-ai-api rather than a provider's theoretical limit.
const DEFAULT_CONTEXT_WINDOW = 24_000
const DEFAULT_MAX_TOKENS = 2_400

export function createPiModel(config: BoxPlayerAgentModelConfig): Model<'openai-completions'> {
  const isCloud = config.providerName === 'boxplayer-cloud'
  const baseUrl = resolveAgentModelEndpoint({ providerName: config.providerName, endpoint: config.endpoint, cloudBaseUrl: Config.BOXPLAYER_AI_API_URL, allowInsecureHttp: true })

  return {
    id: config.modelId,
    name: config.modelId,
    api: 'openai-completions',
    provider: config.providerName || 'openai-compatible',
    baseUrl,
    reasoning: false,
    input: ['text'],
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
    contextWindow: DEFAULT_CONTEXT_WINDOW,
    maxTokens: DEFAULT_MAX_TOKENS
  }
}

export async function resolvePiApiKey(config: BoxPlayerAgentModelConfig): Promise<string | undefined> {
  if (config.providerName === 'boxplayer-cloud') return getBoxPlayerAccessToken()
  return config.apiKey || undefined
}

export function addAgentFeatureToPayload(payload: unknown, surface: AgentSurface): unknown {
  if (!payload || typeof payload !== 'object' || Array.isArray(payload)) return payload
  const feature = surface === 'ai_search' || surface === 'workspace' ? 'ai_search' : surface === 'document' ? 'document_analysis' : 'reader_chat'
  return { ...(payload as Record<string, unknown>), feature }
}
