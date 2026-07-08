export { AIGatewayProvider } from './AIGatewayProvider'
export { OllamaProvider } from './OllamaProvider'
export { OpenRouterProvider, fetchOpenRouterModels } from './OpenRouterProvider'
export type { OpenRouterModelInfo } from './OpenRouterProvider'
export { BoxPlayerCloudProvider } from './BoxPlayerCloudProvider'

import { AIGatewayProvider } from './AIGatewayProvider'
import { OllamaProvider } from './OllamaProvider'
import { OpenRouterProvider } from './OpenRouterProvider'
import { BoxPlayerCloudProvider } from './BoxPlayerCloudProvider'
import type { AISettings, AIProvider } from '../types'

export function getAIProvider(settings: AISettings): AIProvider {
  switch (settings.provider) {
    case 'ollama':
      return new OllamaProvider(settings)
    case 'ai-gateway':
      return new AIGatewayProvider(settings)
    case 'boxplayer-cloud':
      return new BoxPlayerCloudProvider(settings)
    default:
      // All other providers (openai, deepseek, qwen, zhipu, moonshot, siliconflow, openrouter, custom)
      // use OpenAI-compatible API via createOpenAICompatible
      return new OpenRouterProvider(settings)
  }
}
