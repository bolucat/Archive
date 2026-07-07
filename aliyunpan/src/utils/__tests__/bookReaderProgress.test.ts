import { describe, expect, it } from 'vitest'
import { estimateReaderPageProgressFromElement, normalizeReaderPageProgress } from '../bookReaderProgress'

describe('normalizeReaderPageProgress', () => {
  it('uses Koodo getProgress current and total page fields', () => {
    expect(normalizeReaderPageProgress({ currentPage: 12, totalPage: 80 })).toEqual({
      currentPage: 12,
      totalPage: 80,
      percentage: undefined
    })
  })

  it('falls back to the recorded position page', () => {
    expect(normalizeReaderPageProgress(undefined, { page: '7 / 120' })).toEqual({
      currentPage: 7,
      totalPage: 120,
      percentage: undefined
    })
  })

  it('does not expose invalid placeholder page values', () => {
    expect(normalizeReaderPageProgress({ currentPage: 0, totalPage: undefined }, { page: '' }).currentPage).toBe(0)
  })

  it('estimates a visible page from the scroll container when Koodo does not expose page fields', () => {
    expect(estimateReaderPageProgressFromElement({ clientHeight: 800, scrollHeight: 1800, scrollTop: 850 } as HTMLElement)).toEqual({
      currentPage: 2,
      totalPage: 3
    })
  })
})
