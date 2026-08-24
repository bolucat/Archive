import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { IBookItem } from '../../types/book'
import type { IBookNote } from '../../types/bookNote'
import type { IBookBookmark } from '../../types/bookBookmark'

const dbMock = vi.hoisted(() => ({
  deleteBookBookmarksByBookIds: vi.fn(),
  deleteBookItemsByIds: vi.fn(),
  deleteBookNotesByBookIds: vi.fn(),
  getBookItemCounts: vi.fn(),
  getBookItemsByIds: vi.fn(),
  getBookItemsPage: vi.fn(),
  getAllBookItems: vi.fn(),
  getAllBookBookmarks: vi.fn(),
  getAllBookNotes: vi.fn(),
  saveBookItems: vi.fn(),
  getLibrarySources: vi.fn(),
  saveLibrarySource: vi.fn(),
  deleteBookLibrarySource: vi.fn()
}))

const userDalMock = vi.hoisted(() => ({
  GetUserTokenFromDB: vi.fn()
}))

const aliHttpMock = vi.hoisted(() => ({
  GetBlob: vi.fn(),
  IsSuccess: vi.fn(() => true)
}))

const metadataLookupMock = vi.hoisted(() => vi.fn(async () => ({ status: 'no-match' })))

vi.mock('../../utils/db', () => ({
  default: dbMock
}))

vi.mock('../../user/userdal', () => ({
  default: userDalMock
}))

vi.mock('../../aliapi/alihttp', () => ({
  default: aliHttpMock
}))

vi.mock('../../utils/bookExternalMetadata', () => ({
  buildExternalBookMetadataOutcomePatch: (result: any) => result.status === 'matched' ? result.metadata : result.status === 'no-match' ? { metadata_source: 'googlebooks-no-match', metadata_updated_at: Date.now() } : null,
  canHydrateExternalBookMetadata: (item: IBookItem) => !item.cover_url && !item.thumbnail,
  lookupExternalBookMetadata: metadataLookupMock,
  lookupExternalBookMetadataResult: metadataLookupMock
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
    vi.stubGlobal('self', globalThis)
    vi.stubGlobal('localStorage', {
      getItem: vi.fn(() => null),
      setItem: vi.fn()
    })
    dbMock.deleteBookBookmarksByBookIds.mockResolvedValue(0)
    dbMock.deleteBookItemsByIds.mockResolvedValue(0)
    dbMock.deleteBookNotesByBookIds.mockResolvedValue(0)
    dbMock.getAllBookBookmarks.mockResolvedValue([])
    dbMock.getAllBookNotes.mockResolvedValue([])
    dbMock.getAllBookItems.mockResolvedValue([])
    dbMock.getLibrarySources.mockResolvedValue([])
    dbMock.getBookItemsByIds.mockImplementation(async (ids: string[]) => (await dbMock.getAllBookItems()).filter((item: IBookItem) => ids.includes(item.id)))
    dbMock.getBookItemsPage.mockImplementation(async (offset: number, limit: number) => (await dbMock.getAllBookItems()).slice(offset, offset + limit))
    dbMock.getBookItemCounts.mockImplementation(async () => {
      const items = await dbMock.getAllBookItems()
      return { total: items.length, deleted: items.filter((item: IBookItem) => item.deleted_at).length }
    })
    dbMock.saveBookItems.mockResolvedValue(undefined)
    dbMock.saveLibrarySource.mockResolvedValue('book|user|drive|folder')
    dbMock.deleteBookLibrarySource.mockResolvedValue(0)
    userDalMock.GetUserTokenFromDB.mockResolvedValue(undefined)
    aliHttpMock.GetBlob.mockResolvedValue({ code: 200, body: new Blob() })
    metadataLookupMock.mockResolvedValue({ status: 'no-match' })
  })

  it('keeps an empty persisted source visible and deletes it independently of the loaded page', async () => {
    dbMock.getLibrarySources.mockResolvedValue([{
      id: 'book|user|drive|folder',
      kind: 'book',
      user_id: 'user',
      drive_id: 'drive',
      folder_id: 'folder',
      name: 'Books',
      created_at: 1,
      scanned_at: 2
    }])
    const store = await createStore()
    await store.loadFromDB()

    expect(store.byFolder).toEqual([
      expect.objectContaining({ source_id: 'book|user|drive|folder', name: 'Books', count: 0 })
    ])

    await store.deleteSource('book|user|drive|folder')

    expect(dbMock.deleteBookLibrarySource).toHaveBeenCalledWith('book|user|drive|folder')
    expect(store.byFolder).toEqual([])
  })

  it('updates a persisted book when the reader window has not loaded the library list', async () => {
    const saved = book({ reading_progress: 10 })
    dbMock.getBookItemsByIds.mockResolvedValue([saved])
    const store = await createStore()

    await store.updateBookMetadata(saved.id, { reading_progress: 55, reading_position: { page: '11 / 20' } })

    expect(dbMock.saveBookItems).toHaveBeenCalledWith([
      expect.objectContaining({
        id: saved.id,
        reading_progress: 55,
        reading_position: { page: '11 / 20' }
      })
    ])
  })

  it('keeps a book that still belongs to another scanned source', async () => {
    const store = await createStore()
    store.books = [book({ id: 'shared', source_id: 'source-a', source_ids: ['source-a', 'source-b'] })]

    await store.deleteSource('source-a')

    expect(store.books).toEqual([
      expect.objectContaining({ id: 'shared', source_id: 'source-b', source_ids: ['source-b'] })
    ])
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

  it('does not render cached Aliyun thumbnails for a removed account', async () => {
    dbMock.getAllBookItems.mockResolvedValue([
      book({
        id: 'stale-aliyun-book',
        user_id: 'removed-aliyun-user',
        thumbnail: 'https://api.aliyundrive.com/v2/file/download?t=1&drive_id=55307005&file_id=file&office_thumbnail_process=image',
        cover_url: 'https://api.aliyundrive.com/v2/file/download?t=1&drive_id=55307005&file_id=file&office_thumbnail_process=image'
      })
    ])

    const store = await createStore()
    await store.loadFromDB()

    expect(store.books[0]).toMatchObject({ thumbnail: '', cover_url: '' })
    expect(dbMock.saveBookItems).toHaveBeenCalledWith([
      expect.objectContaining({ id: 'stale-aliyun-book', thumbnail: expect.stringContaining('api.aliyundrive.com') })
    ])
  })

  it('loads a cached Aliyun thumbnail with the book owner token', async () => {
    const url = 'https://api.aliyundrive.com/v2/file/download?t=1&drive_id=55307005&file_id=file&office_thumbnail_process=image'
    userDalMock.GetUserTokenFromDB.mockResolvedValue({ tokenfrom: 'aliyun', access_token: 'book-owner-token' })
    aliHttpMock.GetBlob.mockResolvedValue({ code: 200, body: new Blob(['cover'], { type: 'image/jpeg' }) })
    dbMock.getAllBookItems.mockResolvedValue([
      book({ id: 'aliyun-book', user_id: 'book-owner', thumbnail: url, cover_url: url })
    ])

    const store = await createStore()
    await store.loadFromDB()
    await vi.waitFor(() => expect(store.books[0].cover_url).toMatch(/^blob:/))
    expect(aliHttpMock.GetBlob).toHaveBeenCalledWith(url, 'book-owner')
    expect(store.books[0].thumbnail).toMatch(/^blob:/)
  })

  it('queues every coverless book on a loaded page for metadata hydration', async () => {
    dbMock.getAllBookItems.mockResolvedValue(Array.from({ length: 30 }, (_, index) => book({ id: `book-${index}`, file_id: `file-${index}`, file_name: `Book ${index}.epub`, title: `Book ${index}` })))

    const store = await createStore()
    await store.loadFromDB()

    await vi.waitFor(() => expect(metadataLookupMock).toHaveBeenCalledTimes(30))
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
    ], { addToLoaded: false })

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

  it('does not report a scanned book as stored when the database read fails', async () => {
    dbMock.getBookItemsByIds.mockRejectedValueOnce(new Error('IndexedDB unavailable'))
    const store = await createStore()

    await expect(store.appendBooks([book({ id: 'failed-book' })], { addToLoaded: false })).rejects.toThrow('IndexedDB unavailable')
    expect(dbMock.saveBookItems).not.toHaveBeenCalled()
    expect(store.books).toEqual([])
  })

  it('loads book records in pages instead of retaining the entire database at startup', async () => {
    dbMock.getAllBookItems.mockResolvedValue(Array.from({ length: 241 }, (_, index) => book({ id: `book-${index}` })))

    const store = await createStore()
    await store.loadFromDB()

    expect(store.books).toHaveLength(240)
    expect(store.totalCount).toBe(241)
    expect(store.hasMoreBooks).toBe(true)

    await store.loadNextPage()

    expect(store.books).toHaveLength(241)
    expect(store.hasMoreBooks).toBe(false)
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
