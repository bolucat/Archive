import { beforeEach, describe, expect, it, vi } from 'vitest'
import { aiTranslator } from '../translators/ai'

const { generateAIText, getAIConfig } = vi.hoisted(() => ({
  generateAIText: vi.fn(),
  getAIConfig: vi.fn(),
}))

vi.mock('../bookAI', () => ({
  generateAIText,
  getAIConfig,
}))

describe('aiTranslator', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    getAIConfig.mockReturnValue({
      endpoint: '',
      modelId: 'google/gemini-2.5-flash-lite',
      apiKey: 'gateway-key',
      providerName: 'ai-gateway',
    })
    generateAIText.mockResolvedValue('Hello')
  })

  it('uses the shared Vercel AI SDK generation path instead of raw chat fetch', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch')

    await expect(aiTranslator.translate('你好', 'en')).resolves.toBe('Hello')

    expect(generateAIText).toHaveBeenCalledWith(
      {
        endpoint: '',
        modelId: 'google/gemini-2.5-flash-lite',
        apiKey: 'gateway-key',
        providerName: 'ai-gateway',
      },
      expect.stringContaining('请将以下文字翻译成English'),
      1000
    )
    expect(fetchSpy).not.toHaveBeenCalled()
  })

  it('throws a clear error when AI is not configured', async () => {
    getAIConfig.mockReturnValue(null)

    await expect(aiTranslator.translate('你好', 'en')).rejects.toThrow('AI 未配置')
  })
})
