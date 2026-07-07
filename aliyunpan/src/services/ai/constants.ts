import type { AISettings } from './types'

export const GATEWAY_MODELS = {
  GEMINI_FLASH_LITE: 'google/gemini-2.5-flash-lite',
  GPT_5_NANO: 'openai/gpt-5-nano',
  LLAMA_4_SCOUT: 'meta-llama/llama-4-scout',
  GROK_4_FAST: 'x-ai/grok-4.1-fast',
  DEEPSEEK_V3: 'deepseek/deepseek-chat',
  QWEN_3_235B: 'qwen/qwen3-235b-a22b',
} as const

export const MODEL_PRICING: Record<string, { input: string; output: string }> = {
  [GATEWAY_MODELS.GEMINI_FLASH_LITE]: { input: 'Free', output: 'Free' },
  [GATEWAY_MODELS.GPT_5_NANO]: { input: '$0.15', output: '$0.60' },
  [GATEWAY_MODELS.LLAMA_4_SCOUT]: { input: '$0.15', output: '$0.60' },
  [GATEWAY_MODELS.GROK_4_FAST]: { input: '$0.40', output: '$1.60' },
  [GATEWAY_MODELS.DEEPSEEK_V3]: { input: '$0.27', output: '$1.10' },
  [GATEWAY_MODELS.QWEN_3_235B]: { input: '$0.30', output: '$1.20' },
}

export const DEFAULT_AI_SETTINGS: AISettings = {
  provider: 'ai-gateway',
  ollamaUrl: 'http://127.0.0.1:11434',
  ollamaModel: 'llama3.2',
  ollamaEmbeddingModel: 'nomic-embed-text',
  aiGatewayApiKey: '',
  aiGatewayModel: GATEWAY_MODELS.GEMINI_FLASH_LITE,
  aiGatewayEmbeddingModel: 'openai/text-embedding-3-small',
  openRouterApiKey: '',
  openRouterBaseUrl: 'https://openrouter.ai/api/v1',
  openRouterModel: 'openai/gpt-4o-mini',
  openRouterEmbeddingModel: 'openai/text-embedding-3-small',
  spoilerProtection: true,
  maxContextChunks: 10,
  indexingMode: 'on-demand',
  reedy: { enabled: false, runtime: '' },
}
