import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { IBookItem } from '../../types/book'
import type { IBookNote } from '../../types/bookNote'
import type { IBookBookmark } from '../../types/bookBookmark'

const dbMock = vi.hoisted(() => ({
  deleteBookBookmarksByBookIds: vi.fn(),
  deleteBookItemsByIds: vi.fn(),
  deleteBookNotesByBookIds: vi.fn(),
  getAllBookBookmarks: vi.fn(),
  getAllBookNotes: vi.fn(),
  saveBookItems: vi.fn()
}))

vi.mock('../../utils/db', () => ({
  default: dbMock
}))

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

async function createStore() {
  setActivePinia(createPinia())
  const { default: useBookLibraryStore } = await import('../../store/booklibrary')
  return useBookLibraryStore()
}

describe('booklibrary trash behavior', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.stubGlobal('localStorage', {
      getItem: vi.fn(() => null),
      setItem: vi.fn()
    })
    dbMock.deleteBookBookmarksByBookIds.mockResolvedValue(0)
    dbMock.deleteBookItemsByIds.mockResolvedValue(0)
    dbMock.deleteBookNotesByBookIds.mockResolvedValue(0)
    dbMock.getAllBookBookmarks.mockResolvedValue([])
    dbMock.getAllBookNotes.mockResolvedValue([])
    dbMock.saveBookItems.mockResolvedValue(undefined)
  })

  it('moves books to trash and removes favorite state like Reader', async () => {
    const store = await createStore()
    store.books = [
      book({ id: 'deleted', is_favorite: true }),
      book({ id: 'active', is_favorite: true })
    ]

    await store.moveBooksToTrash(['deleted'])

    expect(store.activeBooks.map((item) => item.id)).toEqual(['active'])
    expect(store.deletedBooks.map((item) => item.id)).toEqual(['deleted'])
    expect(store.deletedBooks[0].is_favorite).toBe(false)
    expect(dbMock.saveBookItems).toHaveBeenCalledWith([
      expect.objectContaining({ id: 'deleted', deleted_at: expect.any(Number), is_favorite: false })
    ])
  })

  it('permanently deletes books and clears attached notes and bookmarks', async () => {
    const store = await createStore()
    store.books = [
      book({ id: 'deleted', deleted_at: 10 }),
      book({ id: 'active' })
    ]
    store.notesByBookId = {
      deleted: [note({ id: 'note-deleted', book_id: 'deleted' })],
      active: [note({ id: 'note-active', book_id: 'active' })]
    }
    store.bookmarksByBookId = {
      deleted: [bookmark({ id: 'bookmark-deleted', book_id: 'deleted' })],
      active: [bookmark({ id: 'bookmark-active', book_id: 'active' })]
    }

    await store.deleteBooksByIds(['deleted'])

    expect(dbMock.deleteBookNotesByBookIds).toHaveBeenCalledWith(['deleted'])
    expect(dbMock.deleteBookBookmarksByBookIds).toHaveBeenCalledWith(['deleted'])
    expect(dbMock.deleteBookItemsByIds).toHaveBeenCalledWith(['deleted'])
    expect(store.books.map((item) => item.id)).toEqual(['active'])
    expect(Object.keys(store.notesByBookId)).toEqual(['active'])
    expect(Object.keys(store.bookmarksByBookId)).toEqual(['active'])
  })

  it('does not resurrect deleted books when scanning finds the same file again', async () => {
    const store = await createStore()
    store.books = [
      book({ id: 'deleted', deleted_at: 10, title: 'Deleted copy' }),
      book({ id: 'active', title: 'Old title' })
    ]

    await store.appendBooks([
      book({ id: 'deleted', title: 'Scanned again' }),
      book({ id: 'active', title: 'Updated title' })
    ])

    expect(dbMock.saveBookItems).toHaveBeenCalledWith([
      expect.objectContaining({ id: 'active', title: 'Updated title' })
    ])
    expect(store.books.find((item) => item.id === 'deleted')).toEqual(expect.objectContaining({
      deleted_at: 10,
      title: 'Deleted copy'
    }))
    expect(store.books.find((item) => item.id === 'active')).toEqual(expect.objectContaining({
      title: 'Updated title'
    }))
    expect(store.deletedBooks.map((item) => item.id)).toEqual(['deleted'])
    expect(store.activeBooks.map((item) => item.id)).toEqual(['active'])
  })

  it('loads global annotations for active books without requiring each book to be opened', async () => {
    const store = await createStore()
    store.books = [
      book({ id: 'active' }),
      book({ id: 'deleted', deleted_at: 10 })
    ]
    dbMock.getAllBookNotes.mockResolvedValue([
      note({ id: 'active-note', book_id: 'active', kind: 'note', note: 'typed note', updated_at: 30 }),
      note({ id: 'active-highlight', book_id: 'active', kind: 'highlight', note: '', updated_at: 20 }),
      note({ id: 'deleted-note', book_id: 'deleted', kind: 'note', note: 'deleted note', updated_at: 40 })
    ])
    dbMock.getAllBookBookmarks.mockResolvedValue([
      bookmark({ id: 'active-bookmark', book_id: 'active', updated_at: 30 }),
      bookmark({ id: 'deleted-bookmark', book_id: 'deleted', updated_at: 40 })
    ])

    await store.loadAllBookAnnotations()

    expect(store.allNotes.map((item) => item.id)).toEqual(['active-note'])
    expect(store.allHighlights.map((item) => item.id)).toEqual(['active-highlight'])
    expect(store.allBookmarks.map((item) => item.id)).toEqual(['active-bookmark'])
    expect(store.notesByBookId).toEqual({
      active: [
        expect.objectContaining({ id: 'active-note' }),
        expect.objectContaining({ id: 'active-highlight' })
      ]
    })
    expect(store.bookmarksByBookId).toEqual({
      active: [expect.objectContaining({ id: 'active-bookmark' })]
    })
  })
})
