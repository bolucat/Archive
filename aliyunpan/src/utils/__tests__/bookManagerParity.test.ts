import { describe, expect, it } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'
import type { IBookItem } from '../../types/book'
import type { IBookBookmark } from '../../types/bookBookmark'
import type { IBookNote } from '../../types/bookNote'
import {
  addAnnotationTag,
  buildShelfGroups,
  buildDefaultShelfName,
  buildNoteLink,
  filterAnnotations,
  filterBookmarks,
  DEFAULT_SHELF_ID,
  defaultBookSortOrder,
  filterBooksForManagerView,
  getAnnotationTags,
  getBookManagerTabs,
  normalizeBookSortMode,
  normalizeBookSortOrder,
  removeAnnotationTag,
  renameAnnotationTag,
  shelfNameFromId,
  sortBooksForManagerView,
  sortAnnotations,
  toggleBookId
} from '../bookManagerParity'

function book(overrides: Partial<IBookItem>): IBookItem {
  return {
    id: 'book',
    user_id: 'user',
    drive_id: 'drive',
    file_id: 'file',
    parent_file_id: 'parent',
    file_name: 'Untitled.epub',
    ext: 'epub',
    size: 1,
    category: 'book',
    scanned_at: 1,
    ...overrides
  }
}

function note(overrides: Partial<IBookNote>): IBookNote {
  return {
    id: 'note',
    book_id: 'book',
    user_id: 'user',
    drive_id: 'drive',
    file_id: 'file',
    kind: 'note',
    text: 'text',
    note: 'note',
    chapter: 'chapter',
    chapter_index: 0,
    position: { percentage: 0 },
    range: '',
    color: 0,
    tags: [],
    created_at: 1,
    updated_at: 1,
    ...overrides
  }
}

function bookmark(overrides: Partial<IBookBookmark>): IBookBookmark {
  return {
    id: 'bookmark',
    book_id: 'book',
    user_id: 'user',
    drive_id: 'drive',
    file_id: 'file',
    label: 'bookmark',
    chapter: 'chapter',
    position: { percentage: 0 },
    percentage: 0,
    created_at: 1,
    updated_at: 1,
    ...overrides
  }
}

describe('readerManagerParity', () => {
  it('returns Reader manager tabs in display order', () => {
    expect(getBookManagerTabs().map((tab) => tab.key)).toEqual([
      'home',
      'recent',
      'favorites',
      'shelves',
      'notes',
      'highlights',
      'bookmarks',
      'trash',
      'folders',
      'formats'
    ])
  })

  it('filters favorite books', () => {
    const books = [
      book({ id: 'a', is_favorite: true }),
      book({ id: 'b', is_favorite: false }),
      book({ id: 'c' })
    ]

    expect(filterBooksForManagerView(books, { view: 'favorites' }).map((item) => item.id)).toEqual(['a'])
  })

  it('filters recent books and sorts by latest read first', () => {
    const books = [
      book({ id: 'old', last_read_at: 10 }),
      book({ id: 'unread' }),
      book({ id: 'new', last_read_at: 30 })
    ]

    expect(filterBooksForManagerView(books, { view: 'recent' }).map((item) => item.id)).toEqual(['new', 'old'])
  })

  it('filters shelf, format, and folder views', () => {
    const books = [
      book({ id: 'shelf-a', shelf_id: 'work', ext: 'EPUB', parent_path: '/B' }),
      book({ id: 'shelf-b', shelf_id: 'personal', ext: 'pdf', parent_path: '/A' }),
      book({ id: 'shelf-c', shelf_id: 'work', ext: 'mobi', parent_path: '/B' })
    ]

    expect(filterBooksForManagerView(books, { view: 'shelf', shelfId: 'work' }).map((item) => item.id)).toEqual(['shelf-a', 'shelf-c'])
    expect(filterBooksForManagerView(books, { view: 'format', format: 'PDF' }).map((item) => item.id)).toEqual(['shelf-b'])
    expect(filterBooksForManagerView(books, { view: 'folder', folderPath: '/B' }).map((item) => item.id)).toEqual(['shelf-a', 'shelf-c'])
  })

  it('returns no books for missing selector values', () => {
    const books = [
      book({ id: 'empty-shelf', ext: '', parent_path: '' }),
      book({ id: 'matched', shelf_id: 'work', ext: 'pdf', parent_path: '/B' })
    ]

    expect(filterBooksForManagerView(books, { view: 'shelf' } as Parameters<typeof filterBooksForManagerView>[1])).toEqual([])
    expect(filterBooksForManagerView(books, { view: 'folder', folderPath: '' } as Parameters<typeof filterBooksForManagerView>[1])).toEqual([])
    expect(filterBooksForManagerView(books, { view: 'format', format: '' } as Parameters<typeof filterBooksForManagerView>[1])).toEqual([])
  })

  it('returns a fresh array for home view', () => {
    const books = [book({ id: 'a' }), book({ id: 'b' })]
    const filteredBooks = filterBooksForManagerView(books, { view: 'home' })

    expect(filteredBooks).toEqual(books)
    expect(filteredBooks).not.toBe(books)
  })

  it('sorts books for Reader manager views without mutating input', () => {
    const books = [
      book({ id: 'b', title: 'Beta', author: 'Zephyr', scanned_at: 10, last_read_at: 20, reading_progress: 50, reading_time: 300, size: 20 }),
      book({ id: 'a', title: 'Alpha', author: 'Young', scanned_at: 30, last_read_at: 10, reading_progress: 90, reading_time: 100, size: 10 }),
      book({ id: 'c', file_name: 'Gamma.epub', author: 'Adams', scanned_at: 20, last_read_at: 40, reading_progress: 10, reading_time: 500, size: 30 })
    ]

    expect(sortBooksForManagerView(books, 'title').map((item) => item.id)).toEqual(['a', 'b', 'c'])
    expect(sortBooksForManagerView(books, 'title', 'desc').map((item) => item.id)).toEqual(['c', 'b', 'a'])
    expect(sortBooksForManagerView(books, 'author').map((item) => item.id)).toEqual(['c', 'a', 'b'])
    expect(sortBooksForManagerView(books, 'added').map((item) => item.id)).toEqual(['a', 'c', 'b'])
    expect(sortBooksForManagerView(books, 'recent').map((item) => item.id)).toEqual(['c', 'b', 'a'])
    expect(sortBooksForManagerView(books, 'readingTime').map((item) => item.id)).toEqual(['c', 'b', 'a'])
    expect(sortBooksForManagerView(books, 'progress').map((item) => item.id)).toEqual(['a', 'b', 'c'])
    expect(sortBooksForManagerView(books, 'size', 'asc').map((item) => item.id)).toEqual(['a', 'b', 'c'])
    expect(books.map((item) => item.id)).toEqual(['b', 'a', 'c'])
  })

  it('normalizes Reader sort mode and order', () => {
    expect(defaultBookSortOrder('title')).toBe('asc')
    expect(defaultBookSortOrder('progress')).toBe('desc')
    expect(normalizeBookSortMode('size')).toBe('size')
    expect(normalizeBookSortMode('unknown')).toBe('added')
    expect(normalizeBookSortOrder('asc', 'recent')).toBe('asc')
    expect(normalizeBookSortOrder('bad', 'author')).toBe('asc')
  })

  it('toggles book ids without duplicates', () => {
    expect(toggleBookId(['a', 'b'], 'a')).toEqual(['b'])
    expect(toggleBookId(['a', 'b'], 'c')).toEqual(['a', 'b', 'c'])
    expect(toggleBookId(['a', 'b', 'b'], 'b')).toEqual(['a'])
  })

  it('filters Reader annotations by tags and book', () => {
    const notes = [
      note({ id: 'a', book_id: 'book-a', tags: ['idea', 'quote'] }),
      note({ id: 'b', book_id: 'book-a', tags: ['idea'] }),
      note({ id: 'c', book_id: 'book-b', tags: ['quote', ' reread ', ''] }),
      note({ id: 'd', book_id: 'book-b', tags: [] })
    ]

    expect(getAnnotationTags(notes)).toEqual(['idea', 'quote', 'reread'])
    expect(filterAnnotations(notes).map((item) => item.id)).toEqual(['a', 'b', 'c', 'd'])
    expect(filterAnnotations(notes, { tags: ['idea'] }).map((item) => item.id)).toEqual(['a', 'b'])
    expect(filterAnnotations(notes, { tags: [''] }).map((item) => item.id)).toEqual(['a', 'b', 'c', 'd'])
    expect(filterAnnotations(notes, { tags: ['idea', 'quote'] }).map((item) => item.id)).toEqual(['a'])
    expect(filterAnnotations(notes, { bookId: 'book-b' }).map((item) => item.id)).toEqual(['c', 'd'])
    expect(filterAnnotations(notes, { tags: ['quote'], bookId: 'book-b' }).map((item) => item.id)).toEqual(['c'])
    expect(filterAnnotations(notes, { keyword: 'reread' }).map((item) => item.id)).toEqual(['c'])
    expect(filterAnnotations([
      note({ id: 'text', text: 'Reader excerpt', note: '', chapter: '' }),
      note({ id: 'note', text: '', note: 'Reader memo', chapter: '' }),
      note({ id: 'chapter', text: '', note: '', chapter: 'Finale' })
    ], { keyword: 'memo' }).map((item) => item.id)).toEqual(['note'])
  })

  it('sorts Reader annotations by date or reading progress without mutating input', () => {
    const notes = [
      note({ id: 'middle', created_at: 20, updated_at: 20, position: { percentage: 0.5 } }),
      note({ id: 'old', created_at: 10, updated_at: 10, position: { percentage: 0.8 } }),
      note({ id: 'new', created_at: 30, updated_at: 30, position: { percentage: 0.2 } })
    ]

    expect(sortAnnotations(notes).map((item) => item.id)).toEqual(['new', 'middle', 'old'])
    expect(sortAnnotations(notes, 'date', 'asc').map((item) => item.id)).toEqual(['old', 'middle', 'new'])
    expect(sortAnnotations(notes, 'progress', 'asc').map((item) => item.id)).toEqual(['new', 'middle', 'old'])
    expect(sortAnnotations(notes, 'progress', 'desc').map((item) => item.id)).toEqual(['old', 'middle', 'new'])
    expect(notes.map((item) => item.id)).toEqual(['middle', 'old', 'new'])
  })

  it('builds Reader note deep links', () => {
    expect(buildNoteLink('note-1')).toBe('reader-reader://open-note?noteKey=note-1')
    expect(buildNoteLink('note with spaces')).toBe('reader-reader://open-note?noteKey=note%20with%20spaces')
  })

  it('filters Reader bookmarks by keyword', () => {
    const bookmarks = [
      bookmark({ id: 'a', label: 'Important page', chapter: 'Intro', percentage: 0.1 }),
      bookmark({ id: 'b', label: 'Later', chapter: 'Finale', percentage: 0.45 })
    ]

    expect(filterBookmarks(bookmarks)).toEqual(bookmarks)
    expect(filterBookmarks(bookmarks)).not.toBe(bookmarks)
    expect(filterBookmarks(bookmarks, 'important').map((item) => item.id)).toEqual(['a'])
    expect(filterBookmarks(bookmarks, 'finale').map((item) => item.id)).toEqual(['b'])
    expect(filterBookmarks(bookmarks, '45%').map((item) => item.id)).toEqual(['b'])
  })

  it('builds a default shelf name from user input', () => {
    expect(buildDefaultShelfName('')).toBe('默认书架')
    expect(buildDefaultShelfName('科幻')).toBe('科幻')
    expect(buildDefaultShelfName('  工作  ')).toBe('工作')
  })

  it('groups books into Reader shelves with a default shelf first', () => {
    const books = [
      book({ id: 'a', scanned_at: 10 }),
      book({ id: 'b', shelf_id: '科幻', scanned_at: 20, updated_at: 30 }),
      book({ id: 'c', shelf_id: '科幻', scanned_at: 25, updated_at: 40 }),
      book({ id: 'd', shelf_id: '随笔', scanned_at: 15, updated_at: 35 })
    ]

    const shelves = buildShelfGroups(books)

    expect(shelves.map((shelf) => shelf.id)).toEqual([DEFAULT_SHELF_ID, '科幻', '随笔'])
    expect(shelves[0].name).toBe('默认书架')
    expect(shelves[1].book_ids).toEqual(['b', 'c'])
    expect(shelfNameFromId()).toBe('默认书架')
    expect(shelfNameFromId('工作')).toBe('工作')
  })
})

describe('Koodo book item component contracts', () => {
  const read = (file: string) => fs.readFileSync(path.join(process.cwd(), file), 'utf8')

  it('book item components expose Koodo action buttons', () => {
    for (const file of ['src/layout/BookCardItem.vue', 'src/layout/BookCoverItem.vue', 'src/layout/BookListItem.vue']) {
      const source = read(file)
      expect(source).toContain('favorite')
      expect(source).toContain('detail')
      expect(source).toContain('delete')
      expect(source).toContain('shelf')
    }
  })
})

describe('Koodo note tag CRUD helpers', () => {
  const notes = [
    { id: 'a', tags: ['idea', 'quote'] },
    { id: 'b', tags: ['idea'] },
    { id: 'c', tags: [] },
  ] as any[]

  it('adds a tag without duplicating it', () => {
    expect(addAnnotationTag(notes[0], 'idea').tags).toEqual(['idea', 'quote'])
    expect(addAnnotationTag(notes[2], 'new').tags).toEqual(['new'])
  })

  it('renames a tag across a note', () => {
    expect(renameAnnotationTag(notes[0], 'idea', 'thought').tags).toEqual(['thought', 'quote'])
  })

  it('removes a tag across a note', () => {
    expect(removeAnnotationTag(notes[0], 'quote').tags).toEqual(['idea'])
  })
})
