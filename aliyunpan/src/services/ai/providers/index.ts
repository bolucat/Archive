export { AIGatewayProvider } from './AIGatewayProvider'
export { OllamaProvider } from './OllamaProvider'
export { OpenRouterProvider, fetchOpenRouterModels } from './OpenRouterProvider'
export type { OpenRouterModelInfo } from './OpenRouterProvider'

import { AIGatewayProvider } from './AIGatewayProvider'
import { OllamaProvider } from './OllamaProvider'
import { OpenRouterProvider } from './OpenRouterProvider'
import type { AISettings, AIProvider } from '../types'

export function getAIProvider(settings: AISettings): AIProvider {
  switch (settings.provider) {
    case 'ollama':
      return new OllamaProvider(settings)
    case 'ai-gateway':
      return new AIGatewayProvider(settings)
    default:
      // All other providers (openai, deepseek, qwen, zhipu, moonshot, siliconflow, openrouter, custom)
      // use OpenAI-compatible API via createOpenAI
      return new OpenRouterProvider(settings)
  }
}
