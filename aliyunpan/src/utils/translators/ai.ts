import type { TranslationProvider } from './types'
import { generateAIText, getAIConfig } from '../bookAI'

const LANG_LABELS: Record<string, string> = {
  zh: '中文', en: 'English', ja: '日本語', ko: '한국어', fr: 'Français',
  de: 'Deutsch', es: 'Español', pt: 'Português', ru: 'Русский', ar: 'العربية',
}

export const aiTranslator: TranslationProvider = {
  name: 'ai',
  label: 'AI 翻译',
  async translate(text: string, targetLang: string): Promise<string> {
    const cfg = getAIConfig()
    if (!cfg) throw new Error('AI 未配置')
    const langLabel = LANG_LABELS[targetLang] || targetLang
    const prompt = `请将以下文字翻译成${langLabel}，只返回译文不要解释：\n\n${text}`
    return generateAIText(cfg, prompt, 1000)
  },
}
