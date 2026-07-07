import { describe, expect, it } from 'vitest'
import { isReaderFormat } from '../bookReaderCapabilities'
import { normalizeBookReaderPreferences } from '../bookReaderPreferences'
import { buildBookAnnotationsMarkdown } from '../bookAnnotationExport'

describe('reader integration smoke helpers', () => {
  it('routes Reader formats and preserves reader preferences/export', () => {
    expect(isReaderFormat('mobi')).toBe(true)
    expect(isReaderFormat('azw3')).toBe(true)
    expect(normalizeBookReaderPreferences({ readerLayoutMode: 'scroll', readerBionic: true }).readerBionic).toBe(true)
    expect(buildBookAnnotationsMarkdown({
      book: { title: 'Smoke', file_name: 'smoke.md', author: 'Tester' },
      notes: [],
      bookmarks: []
    })).toContain('暂无书摘')
  })
})
