type SubtitleLanguageAlias = {
  code: string
  name?: {
    'zh-CN': string
    'en-US': string
  }
}

const aliases: Record<string, SubtitleLanguageAlias> = {
  'simplified, singapore': { code: 'zh-Hans-SG' },
  'simplified, china': { code: 'zh-Hans-CN' },
  simplified: { code: 'zh-Hans' },
  'traditional, hong kong': { code: 'zh-Hant-HK' },
  'traditional, taiwan': { code: 'zh-Hant-TW' },
  traditional: { code: 'zh-Hant' },
  'latin america': { code: 'es-419' },
  brazil: { code: 'pt-BR' },
  malaysia: { code: 'ms-MY' },
  cyrillic: { code: 'und-Cyrl', name: { 'zh-CN': '西里尔文字', 'en-US': 'Cyrillic' } },
  latin: { code: 'und-Latn', name: { 'zh-CN': '拉丁文字', 'en-US': 'Latin' } },
  bul: { code: 'bg' }, cze: { code: 'cs' }, dan: { code: 'da' }, ger: { code: 'de' }, gre: { code: 'el' }, eng: { code: 'en' }, spa: { code: 'es' }, est: { code: 'et' }, fin: { code: 'fi' }, fre: { code: 'fr' }, heb: { code: 'he' }, hrv: { code: 'hr' }, hun: { code: 'hu' }, ind: { code: 'id' }, ice: { code: 'is' }, ita: { code: 'it' }, lit: { code: 'lt' }, lav: { code: 'lv' }, mac: { code: 'mk' }, nob: { code: 'nb' }, dut: { code: 'nl' }, pol: { code: 'pl' }, por: { code: 'pt' }, rum: { code: 'ro' }, run: { code: 'rn' }, slo: { code: 'sk' }, slv: { code: 'sl' }, swe: { code: 'sv' }, tha: { code: 'th' }, tur: { code: 'tr' }, vie: { code: 'vi' }, chi: { code: 'zh' }, zho: { code: 'zh' }, jpn: { code: 'ja' }, kor: { code: 'ko' }
}

const cleanSubtitleLanguage = (value?: string) => {
  let clean = String(value || '').trim()
  let previous = ''
  while (clean && clean !== previous) {
    previous = clean
    clean = clean
      .replace(/^\s*(?:内嵌|嵌入|embedded)\s*[:：]?\s*/i, '')
      .replace(/^\s*\[(?:内置字幕|embedded(?:\s+subtitle)?)\]\s*/i, '')
  }
  return clean.replace(/\s+/g, ' ').trim()
}

/** Converts provider-specific embedded-subtitle labels into a localized name plus a BCP-47 code. */
export const formatEmbeddedSubtitleLabel = (value?: string, locale: 'zh-CN' | 'en-US' = 'zh-CN') => {
  const raw = cleanSubtitleLanguage(value)
  const hearingImpaired = /\s*\((?:SDH|CC)\)\s*$/i.test(raw)
  const language = raw.replace(/\s*\((?:SDH|CC)\)\s*$/i, '').trim()
  if (!language) return hearingImpaired ? 'SDH' : ''

  const key = language.toLowerCase().replace(/_/g, '-').trim()
  const alias = aliases[key]
  const code = alias?.code || language.replace(/_/g, '-')
  const displayName = alias?.name?.[locale] || (() => {
    try {
      return new Intl.DisplayNames([locale], { type: 'language' }).of(code)
    } catch {
      return undefined
    }
  })() || language

  return `${displayName} · ${code}${hearingImpaired ? ' · SDH' : ''}`
}
