export type BookLookupMode = 'translation' | 'dict'

export interface BookLookupLink {
  label: string
  url: string
  primary?: boolean
}

export function normalizeLookupText(text: string): string {
  return (text || '').replace(/\s+/g, ' ').trim()
}

export function getReaderTranslationTarget(language = ''): string {
  return language.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en'
}

export function buildReaderTranslationLinks(text: string, language = ''): BookLookupLink[] {
  const query = normalizeLookupText(text)
  if (!query) return []
  const encoded = encodeURIComponent(query)
  const target = getReaderTranslationTarget(language)
  const deeplTarget = target.startsWith('zh') ? 'zh' : target
  return [
    {
      label: 'Google Translate',
      url: `https://translate.google.com/?sl=auto&tl=${encodeURIComponent(target)}&text=${encoded}&op=translate`,
      primary: true
    },
    {
      label: 'DeepL',
      url: `https://www.deepl.com/translator#auto/${encodeURIComponent(deeplTarget)}/${encoded}`
    },
    {
      label: 'Youdao',
      url: `https://www.youdao.com/result?word=${encoded}&lang=auto`
    }
  ]
}

export function buildReaderDictionaryLinks(text: string, language = ''): BookLookupLink[] {
  const query = normalizeLookupText(text)
  if (!query) return []
  const encoded = encodeURIComponent(query)
  const links: BookLookupLink[] = [
    {
      label: language.toLowerCase().startsWith('zh') ? '有道词典' : 'Cambridge Dictionary',
      url: language.toLowerCase().startsWith('zh')
        ? `https://www.youdao.com/result?word=${encoded}&lang=en`
        : `https://dictionary.cambridge.org/search/english/direct/?q=${encoded}`,
      primary: true
    },
    {
      label: 'Google Define',
      url: `https://www.google.com/search?q=${encodeURIComponent(`define ${query}`)}`
    }
  ]
  if (!language.toLowerCase().startsWith('zh')) {
    links.push({
      label: 'Youdao',
      url: `https://www.youdao.com/result?word=${encoded}&lang=en`
    })
  }
  return links
}

export function buildBookLookupLinks(mode: BookLookupMode, text: string, language = ''): BookLookupLink[] {
  return mode === 'translation'
    ? buildReaderTranslationLinks(text, language)
    : buildReaderDictionaryLinks(text, language)
}
