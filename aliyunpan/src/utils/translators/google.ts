import type { TranslationProvider } from './types'

const LANG_MAP: Record<string, string> = {
  zh: 'zh-CN', en: 'en', ja: 'ja', ko: 'ko', fr: 'fr', de: 'de',
  es: 'es', pt: 'pt', ru: 'ru', ar: 'ar',
}

export const googleTranslator: TranslationProvider = {
  name: 'google',
  label: 'Google 翻译',
  async translate(text: string, targetLang: string): Promise<string> {
    const tl = LANG_MAP[targetLang] || targetLang
    const resp = await fetch(
      `https://translate.googleapis.com/translate_a/single?client=gtx&dt=t&sl=auto&tl=${tl}&q=${encodeURIComponent(text)}`
    )
    const data = await resp.json()
    const segments = data?.[0] || []
    return segments.map((s: any[]) => s[0]).join('')
  },
}
