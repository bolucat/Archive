import { describe, expect, it } from 'vitest'
import {
  buildReaderDictionaryLinks,
  buildBookLookupLinks,
  buildReaderTranslationLinks,
  getReaderTranslationTarget,
  normalizeLookupText
} from '../bookLookup'

describe('readerLookup', () => {
  it('normalizes selected text for lookup popups', () => {
    expect(normalizeLookupText('  hello\nworld\t ')).toBe('hello world')
  })

  it('chooses a default translation target from navigator language', () => {
    expect(getReaderTranslationTarget('zh-CN')).toBe('zh-CN')
    expect(getReaderTranslationTarget('en-US')).toBe('en')
  })

  it('builds external translation links without requiring Reader plugin secrets', () => {
    const links = buildReaderTranslationLinks('Project Hail Mary', 'zh-CN')
    expect(links[0]).toEqual({
      label: 'Google Translate',
      url: 'https://translate.google.com/?sl=auto&tl=zh-CN&text=Project%20Hail%20Mary&op=translate',
      primary: true
    })
    expect(links.map((item) => item.label)).toContain('DeepL')
  })

  it('builds dictionary links for the selected word', () => {
    expect(buildReaderDictionaryLinks(' gravity ', 'en-US')[0]).toEqual({
      label: 'Cambridge Dictionary',
      url: 'https://dictionary.cambridge.org/search/english/direct/?q=gravity',
      primary: true
    })
  })

  it('dispatches lookup link building by Reader popup mode', () => {
    expect(buildBookLookupLinks('dict', 'orbit', 'en-US')[0].label).toBe('Cambridge Dictionary')
    expect(buildBookLookupLinks('translation', '轨道', 'zh-CN')[0].label).toBe('Google Translate')
  })
})
