import type { TranslationProvider, TranslatorRegistry } from './types'
import { azureTranslator } from './azure'
import { googleTranslator } from './google'
import { aiTranslator } from './ai'
import { isAIConfigured } from '../bookAI'

export type { TranslationProvider, TranslatorRegistry } from './types'

const LS_TRANSLATOR = 'bookReader.translator'

const providers: TranslationProvider[] = [googleTranslator, azureTranslator, aiTranslator]

function getDefault(): string {
  return localStorage.getItem(LS_TRANSLATOR) || 'google'
}

export const translators: TranslatorRegistry = {
  providers,
  defaultName: getDefault(),
  get(name: string) {
    if (name === 'ai' && !isAIConfigured()) return undefined
    return providers.find((p) => p.name === name)
  },
}

export function setTranslator(name: string) {
  try { localStorage.setItem(LS_TRANSLATOR, name) } catch {}
}

export async function translateText(text: string, targetLang: string, providerName?: string): Promise<string> {
  const name = providerName || getDefault()
  const provider = translators.get(name)
  if (!provider) {
    const fallback = translators.get('google')!
    return fallback.translate(text, targetLang)
  }
  try {
    return await provider.translate(text, targetLang)
  } catch {
    const fallback = translators.get('google')!
    return fallback.translate(text, targetLang)
  }
}

export const transLanguages = [
  { value: 'zh', label: '中文' },
  { value: 'en', label: 'English' },
  { value: 'ja', label: '日本語' },
  { value: 'ko', label: '한국어' },
  { value: 'fr', label: 'Français' },
  { value: 'de', label: 'Deutsch' },
  { value: 'es', label: 'Español' },
  { value: 'pt', label: 'Português' },
  { value: 'ru', label: 'Русский' },
  { value: 'ar', label: 'العربية' },
]
