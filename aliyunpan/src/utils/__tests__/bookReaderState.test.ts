import { describe, expect, it } from 'vitest'
import {
  buildBookReadingPatch,
  buildBookReadingTimePatch,
  getBookReadingProgressText,
  normalizeReaderPosition
} from '../bookReaderState'

describe('bookReaderState', () => {
  it('normalizes Reader percentage positions for storage', () => {
    expect(normalizeReaderPosition({
      percentage: '0.425',
      chapterTitle: '第二章',
      chapterDocIndex: '2',
      text: 'hello'
    })).toEqual({
      percentage: 0.425,
      chapterTitle: '第二章',
      chapterDocIndex: '2',
      text: 'hello'
    })
  })

  it('builds a persisted reading patch from a Reader position', () => {
    const patch = buildBookReadingPatch({
      percentage: 0.5,
      chapterTitle: 'Chapter 5'
    }, 1710000000000)

    expect(patch.reader_engine).toBeUndefined()
    expect(patch.reading_progress).toBe(50)
    expect(patch.reading_progress_text).toBe('50%')
    expect(patch.reading_chapter).toBe('Chapter 5')
    expect(patch.last_read_at).toBe(1710000000000)
  })

  it('uses page text when percentage is unavailable', () => {
    expect(getBookReadingProgressText({ page: '12 / 88' })).toBe('12 / 88')
  })

  it('builds a cumulative reading time patch in seconds', () => {
    expect(buildBookReadingTimePatch(120, 1_710_000_000_000, 1_710_000_065_400)).toEqual({
      reading_time: 185,
      last_read_at: 1_710_000_065_400
    })
    expect(buildBookReadingTimePatch(undefined, 1_710_000_000_000, 1_710_000_000_900)).toEqual({})
  })
})
