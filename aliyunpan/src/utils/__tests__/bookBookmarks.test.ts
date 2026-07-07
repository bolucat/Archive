import { describe, expect, it } from 'vitest'
import { buildBookBookmark, getBookmarkFingerprint } from '../bookBookmarks'

describe('bookBookmarks', () => {
  it('builds a persisted bookmark from Reader position', () => {
    const bookmark = buildBookBookmark({
      book: { id: 'book-1', user_id: 'u1', drive_id: 'd1', file_id: 'f1' },
      position: {
        cfi: 'epubcfi(/6/2)',
        text: '  hello\nworld  ',
        percentage: 0.42,
        chapterTitle: 'Chapter 3'
      },
      now: 1710000000000
    })

    expect(bookmark.id).toBe('book-1|epubcfi(/6/2)')
    expect(bookmark.label).toBe('hello world')
    expect(bookmark.chapter).toBe('Chapter 3')
    expect(bookmark.percentage).toBe(0.42)
    expect(bookmark.created_at).toBe(1710000000000)
  })

  it('uses percentage as fingerprint when CFI is unavailable', () => {
    expect(getBookmarkFingerprint({ percentage: 0.5, chapterTitle: 'Chapter' })).toBe('0.5')
  })

  it('falls back to chapter/count fingerprint for plain text positions', () => {
    expect(getBookmarkFingerprint({ chapterDocIndex: '2', chapterTitle: 'Chapter', count: '88' }))
      .toBe('{"chapterDocIndex":"2","chapterTitle":"Chapter","count":"88"}')
  })
})
