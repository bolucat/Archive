import type { AIProviderName } from './types'

export function canUseSemanticEmbeddings(provider: AIProviderName): boolean {
  return provider === 'boxplayer-cloud' || provider === 'ollama'
}
