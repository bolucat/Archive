import { describe, expect, it } from 'vitest'
import { normalizeReaderSearchResults } from '../bookReader'

describe('normalizeReaderSearchResults', () => {
  it('parses Reader cfi payload and strips result html', () => {
    const results = normalizeReaderSearchResults([
      {
        text: 'hit-1',
        excerpt: 'before <span class="content-search-text">needle</span> after',
        cfi: JSON.stringify({
          text: 'needle',
          keyword: 'needle',
          chapterTitle: 'Chapter 1',
          chapterDocIndex: '0',
          percentage: '0.25'
        })
      }
    ])

    expect(results).toEqual([
      {
        id: '0-hit-1',
        excerpt: 'before needle after',
        chapterTitle: 'Chapter 1',
        keyword: 'needle',
        position: {
          text: 'needle',
          chapterTitle: 'Chapter 1',
          chapterDocIndex: '0',
          percentage: 0.25
        }
      }
    ])
  })

  it('keeps invalid cfi results navigable with an empty position', () => {
    const results = normalizeReaderSearchResults([{ excerpt: 'plain', cfi: '{bad' }])
    expect(results[0].excerpt).toBe('plain')
    expect(results[0].position).toEqual({})
  })
})
