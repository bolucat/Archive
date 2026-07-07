import type { TranslationProvider } from './types'

let cachedToken = ''
let tokenExpiry = 0

async function getAzureToken(): Promise<string> {
  if (cachedToken && Date.now() < tokenExpiry) return cachedToken
  const resp = await fetch('https://edge.microsoft.com/translate/auth')
  cachedToken = await resp.text()
  tokenExpiry = Date.now() + 8 * 60 * 1000
  return cachedToken
}

const LANG_MAP: Record<string, string> = {
  zh: 'zh-Hans', en: 'en', ja: 'ja', ko: 'ko', fr: 'fr', de: 'de',
  es: 'es', pt: 'pt', ru: 'ru', ar: 'ar',
}

export const azureTranslator: TranslationProvider = {
  name: 'azure',
  label: 'Azure Translator',
  async translate(text: string, targetLang: string): Promise<string> {
    const token = await getAzureToken()
    const to = LANG_MAP[targetLang] || targetLang
    const resp = await fetch(
      `https://api-edge.cognitive.microsofttranslator.com/translate?to=${to}&api-version=3.0`,
      {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify([{ Text: text }]),
      }
    )
    const data = await resp.json()
    return data?.[0]?.translations?.[0]?.text || ''
  },
}
