import { describe, expect, it } from 'vitest'
import { canUseSemanticEmbeddings } from '../../services/ai/embeddingPolicy'

describe('embedding policy', () => {
  it('keeps BYOK providers on local keyword indexing', () => {
    expect(canUseSemanticEmbeddings('deepseek')).toBe(false)
    expect(canUseSemanticEmbeddings('ai-gateway')).toBe(false)
    expect(canUseSemanticEmbeddings('openrouter')).toBe(false)
  })

  it('allows the built-in worker and local Ollama semantic indexes', () => {
    expect(canUseSemanticEmbeddings('boxplayer-cloud')).toBe(true)
    expect(canUseSemanticEmbeddings('ollama')).toBe(true)
  })
})
