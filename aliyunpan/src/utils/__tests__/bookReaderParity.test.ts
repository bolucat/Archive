import { describe, expect, it, vi } from 'vitest'
import { buildInitialReaderPosition, getPanelVisible, nextPanelLockState, normalizeReaderPercentage, serializeReaderPositionForJump } from '../bookReaderParity'
import { createReaderStyleConfig, readRecordedReaderPosition, waitForReaderRender } from '../bookReader'

describe('readerParity', () => {
  it('keeps locked panels visible after hover closes', () => {
    expect(getPanelVisible(false, false)).toBe(false)
    expect(getPanelVisible(true, false)).toBe(true)
    expect(getPanelVisible(false, true)).toBe(true)
  })

  it('toggles panel locks and forces open when locked', () => {
    expect(nextPanelLockState(false)).toEqual({ locked: true, open: true })
    expect(nextPanelLockState(true)).toEqual({ locked: false, open: false })
  })

  it('serializes Reader position for note/bookmark/search jumps', () => {
    expect(serializeReaderPositionForJump({ cfi: 'abc', count: 10 })).toBe('{"cfi":"abc","count":"ignore","isFirst":true}')
    expect(serializeReaderPositionForJump({ count: 10, chapterTitle: 'A' })).toBe('{"count":10,"chapterTitle":"A","isFirst":true}')
  })

  it('builds an explicit first chapter position when no reading position exists', () => {
    expect(buildInitialReaderPosition()).toEqual({
      text: '',
      chapterTitle: '',
      page: '',
      chapterDocIndex: '0',
      chapterHref: '',
      count: 0,
      cfi: ''
    })
    expect(buildInitialReaderPosition({ chapterDocIndex: '2', count: undefined })).toEqual({
      chapterDocIndex: '2',
      count: 0
    })
  })

  it('normalizes Reader progress jumps to a 0-1 percentage', () => {
    expect(normalizeReaderPercentage(0.42)).toBe(0.42)
    expect(normalizeReaderPercentage(-1)).toBe(0)
    expect(normalizeReaderPercentage(2)).toBe(1)
    expect(normalizeReaderPercentage('bad')).toBe(0)
  })

  it('does not block the reader shell when Reader render promise does not settle', async () => {
    vi.useFakeTimers()
    try {
      const waitPromise = waitForReaderRender(new Promise(() => {}))
      await vi.advanceTimersByTimeAsync(1800)

      await expect(waitPromise).resolves.toBeUndefined()
    } finally {
      vi.useRealTimers()
    }
  })

  it('records Reader scroll location before reading the normalized position', async () => {
    const calls: string[] = []
    const rendition = {
      record: vi.fn(async () => {
        calls.push('record')
      }),
      getPosition: vi.fn(() => {
        calls.push('getPosition')
        return { percentage: '0.42', page: 3 }
      })
    }

    await expect(readRecordedReaderPosition(rendition)).resolves.toEqual({
      percentage: 0.42,
      page: '3'
    })
    expect(calls).toEqual(['record', 'getPosition'])
  })

  it('builds a minimal Reader style config adapter for iframe CSS injection', () => {
    const config = createReaderStyleConfig({
      sourceUrl: '/book.epub',
      ext: 'epub',
      container: {} as HTMLElement,
      fontSize: 20,
      lineHeight: '1.75',
      textAlign: 'Justify',
      isIndent: false,
      isHyphenation: true,
      paraSpacingValue: '36',
      backgroundColor: 'rgba(44,47,49,1)',
      textColor: 'rgba(255,255,255,1)'
    })

    expect(config.getReaderConfig('fontSize')).toBe('20')
    expect(config.getReaderConfig('lineHeight')).toBe('1.75')
    expect(config.getReaderConfig('textAlign')).toBe('Justify')
    expect(config.getReaderConfig('isIndent')).toBe('no')
    expect(config.getReaderConfig('isHyphenation')).toBe('yes')
    expect(config.getReaderConfig('paraSpacing')).toBe('36')
    expect(config.getReaderConfig('backgroundColor')).toBe('rgba(44,47,49,1)')
    expect(config.getReaderConfig('textColor')).toBe('rgba(255,255,255,1)')
    expect(config.getReaderConfig('isOverwriteText')).toBe('yes')
    expect(config.getAllListConfig()).toEqual([])
  })
})
