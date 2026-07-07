import { describe, expect, it } from 'vitest'
import {
  HIGHLIGHT_COLORS,
  buildBrowserSearchUrl,
  buildPopupActions,
  getReaderHighlightColor,
  normalizePopupNoteText,
  normalizeReaderHighlightColor
} from '../bookPopupActions'

describe('bookPopupActions', () => {
  it('uses Reader original popup defaults and caps visible actions at eight', () => {
    expect(buildPopupActions().map((item) => item.key)).toEqual([
      'note',
      'highlight',
      'translation',
      'copy',
      'search-book',
      'dict',
      'browser',
      'speaker'
    ])
  })

  it('keeps Reader highlight color indexes compatible with background and line modes', () => {
    expect(HIGHLIGHT_COLORS).toHaveLength(8)
    expect(getReaderHighlightColor(0)).toEqual({ index: 0, value: '#FBF1D1', mode: 'background' })
    expect(getReaderHighlightColor(4)).toEqual({ index: 4, value: '#FF0000', mode: 'line' })
    expect(normalizeReaderHighlightColor(-1)).toBe(0)
    expect(normalizeReaderHighlightColor(99)).toBe(0)
  })

  it('builds Reader default external search urls from selected text', () => {
    expect(buildBrowserSearchUrl('三体 章北海', 'baidu')).toBe('https://www.baidu.com/s?wd=%E4%B8%89%E4%BD%93%20%E7%AB%A0%E5%8C%97%E6%B5%B7')
    expect(buildBrowserSearchUrl('Project Hail Mary', 'google')).toBe('https://www.google.com/search?q=Project%20Hail%20Mary')
    expect(buildBrowserSearchUrl('  ', 'google')).toBe('')
    expect(buildBrowserSearchUrl('test')).toBe('https://www.google.com/search?q=test')
  })

  it('normalizes inline popup note text before creating a note', () => {
    expect(normalizePopupNoteText('  memo\nline  ')).toBe('memo\nline')
    expect(normalizePopupNoteText('   ')).toBe('')
  })
})
