import { describe, expect, it } from 'vitest'
import { buildBookHighlight, toBookNote, updateBookNote } from '../bookNotes'

describe('bookNotes', () => {
  it('builds a persisted highlight from selected text and position', () => {
    const note = buildBookHighlight({
      book: {
        id: 'book-1',
        user_id: 'u1',
        drive_id: 'd1',
        file_id: 'f1'
      },
      text: '  hello\nworld  ',
      position: {
        percentage: 0.3,
        chapterTitle: 'Chapter 2',
        chapterDocIndex: '1'
      },
      range: '[1,2]',
      now: 1710000000000
    })

    expect(note.id).toBe('book-1|1710000000000')
    expect(note.text).toBe('hello world')
    expect(note.chapter).toBe('Chapter 2')
    expect(note.chapter_index).toBe(1)
    expect(note.kind).toBe('highlight')
  })

  it('converts a persisted note to Reader note shape', () => {
    const note = buildBookHighlight({
      book: { id: 'book-1', user_id: 'u1', drive_id: 'd1', file_id: 'f1' },
      text: 'hello',
      position: { percentage: 0.3, chapterTitle: 'Chapter 2', chapterDocIndex: '1' },
      range: '[1,2]',
      now: 1710000000000
    })

    expect(toBookNote(note)).toMatchObject({
      key: note.id,
      bookKey: 'book-1',
      chapter: 'Chapter 2',
      chapterIndex: 1,
      text: 'hello',
      notes: '',
      percentage: '0.3',
      color: 0,
      tag: []
    })
  })

  it('updates note text, color, tags and timestamp without changing highlight identity', () => {
    const note = buildBookHighlight({
      book: { id: 'book-1', user_id: 'u1', drive_id: 'd1', file_id: 'f1' },
      text: 'hello',
      position: { percentage: 0.3, chapterTitle: 'Chapter 2', chapterDocIndex: '1' },
      range: '[1,2]',
      now: 1710000000000
    })

    const updated = updateBookNote(note, {
      note: '  my memo  ',
      color: 2,
      tags: [' idea ', '', 'quote'],
      now: 1710000005000
    })

    expect(updated.id).toBe(note.id)
    expect(updated.text).toBe('hello')
    expect(updated.kind).toBe('note')
    expect(updated.note).toBe('my memo')
    expect(updated.color).toBe(2)
    expect(updated.tags).toEqual(['idea', 'quote'])
    expect(updated.created_at).toBe(note.created_at)
    expect(updated.updated_at).toBe(1710000005000)
  })

  it('downgrades back to a highlight when note text is cleared', () => {
    const note = buildBookHighlight({
      book: { id: 'book-1', user_id: 'u1', drive_id: 'd1', file_id: 'f1' },
      text: 'hello',
      note: 'memo',
      position: { percentage: 0.3, chapterTitle: 'Chapter 2', chapterDocIndex: '1' },
      range: '[1,2]',
      now: 1710000000000
    })

    expect(updateBookNote(note, { note: '   ', now: 1710000005000 }).kind).toBe('highlight')
  })

  it('keeps Reader highlight color when selected text is saved as a note', () => {
    const note = buildBookHighlight({
      book: { id: 'book-1', user_id: 'u1', drive_id: 'd1', file_id: 'f1' },
      text: 'hello',
      note: 'memo',
      color: 6,
      position: { percentage: 0.3, chapterTitle: 'Chapter 2', chapterDocIndex: '1' },
      range: '[1,2]',
      now: 1710000000000
    })

    expect(note.kind).toBe('note')
    expect(note.note).toBe('memo')
    expect(note.color).toBe(6)
    expect(toBookNote(note).color).toBe(6)
  })
})
