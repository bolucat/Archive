import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

const dbMock = vi.hoisted(() => ({
  getMediaLibrary: vi.fn(),
  getMediaLibraryFolders: vi.fn(),
  getMediaLibraryPage: vi.fn(),
  countMediaLibraryItems: vi.fn(),
  saveMediaLibrary: vi.fn(),
  upsertMediaLibraryItems: vi.fn(),
  deleteMediaLibraryItems: vi.fn(),
  upsertMediaLibraryFolders: vi.fn(),
  deleteMediaLibraryFolders: vi.fn(),
  clearMediaLibrary: vi.fn()
}))

vi.mock('../../utils/db', () => ({ default: dbMock }))

class MemoryStorage implements Storage {
  private values = new Map<string, string>()
  get length() { return this.values.size }
  clear() { this.values.clear() }
  getItem(key: string) { return this.values.get(key) ?? null }
  key(index: number) { return [...this.values.keys()][index] ?? null }
  removeItem(key: string) { this.values.delete(key) }
  setItem(key: string, value: string) { this.values.set(key, value) }
}

const item = (id: string, addedAt: string) => ({ id, parentId: '', type: 'movie' as const, name: id, genres: [], driveFiles: [], addedAt: new Date(addedAt) })
const folder = (id: string, scanDate: string) => ({ id, fileId: id, name: id, userId: 'user', driveId: 'drive', driveServerId: 'aliyun', itemCount: 0, scanDate: new Date(scanDate) })

describe('media-library Dexie migration', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.stubGlobal('localStorage', new MemoryStorage())
    vi.stubGlobal('indexedDB', {})
    vi.stubGlobal('BroadcastChannel', class { addEventListener() {} postMessage() {} })
    dbMock.getMediaLibrary.mockResolvedValue({ items: [], folders: [] })
    dbMock.getMediaLibraryFolders.mockResolvedValue([])
    dbMock.getMediaLibraryPage.mockResolvedValue([])
    dbMock.countMediaLibraryItems.mockResolvedValue(0)
    dbMock.saveMediaLibrary.mockResolvedValue(undefined)
    dbMock.upsertMediaLibraryItems.mockResolvedValue(undefined)
    dbMock.deleteMediaLibraryItems.mockResolvedValue(undefined)
    dbMock.upsertMediaLibraryFolders.mockResolvedValue(undefined)
    dbMock.deleteMediaLibraryFolders.mockResolvedValue(undefined)
    setActivePinia(createPinia())
  })

  it('merges legacy localStorage with a partially populated Dexie library, keeping newer records', async () => {
    localStorage.setItem('MediaLibrary_MediaItems', JSON.stringify([item('same', '2026-07-02'), item('legacy-only', '2026-07-01')]))
    localStorage.setItem('MediaLibrary_Folders', JSON.stringify([folder('same-folder', '2026-07-02'), folder('legacy-folder', '2026-07-01')]))
    dbMock.getMediaLibrary.mockResolvedValue({
      items: [item('same', '2026-07-01'), item('db-only', '2026-07-01')],
      folders: [folder('same-folder', '2026-07-01'), folder('db-folder', '2026-07-01')]
    })
    dbMock.getMediaLibraryFolders.mockResolvedValue([
      folder('same-folder', '2026-07-01'), folder('db-folder', '2026-07-01')
    ])
    dbMock.getMediaLibraryPage.mockResolvedValue([
      item('same', '2026-07-02'), item('legacy-only', '2026-07-01'), item('db-only', '2026-07-01')
    ])

    const { useMediaLibraryStore } = await import('../../store/medialibrary')
    const store = useMediaLibraryStore()
    await store.hydrate()

    expect(store.mediaItems.map(entry => entry.id).sort()).toEqual(['db-only', 'legacy-only', 'same'])
    expect(store.mediaItems.find(entry => entry.id === 'same')?.addedAt).toEqual(new Date('2026-07-02'))
    expect(store.folders.map(entry => entry.id).sort()).toEqual(['db-folder', 'legacy-folder', 'same-folder'])
    expect(dbMock.saveMediaLibrary).toHaveBeenCalled()
    expect(localStorage.getItem('MediaLibrary_DexieMigrated_v1')).toBe('1')
  })

  it('does not prune distinct legacy sources that only share the same name', async () => {
    localStorage.setItem('MediaLibrary_DexieMigrated_v1', '1')
    const { useMediaLibraryStore } = await import('../../store/medialibrary')
    const store = useMediaLibraryStore()
    await store.hydrate()
    store.addFolder({ ...folder('first', '2026-07-01'), name: 'Movies', fileId: 'folder-a', path: '/a' })
    store.addFolder({ ...folder('second', '2026-07-01'), name: 'Movies', fileId: 'folder-b', path: '/b' })
    store.addMediaItem({ ...item('movie-a', '2026-07-01'), folderId: 'first' })

    expect(store.pruneOrphanDuplicateFolders()).toBe(0)
    expect(store.folders.map(entry => entry.id).sort()).toEqual(['first', 'second'])
  })

  it('replaces manually edited metadata in the library and recent snapshot without creating a duplicate', async () => {
    localStorage.setItem('MediaLibrary_DexieMigrated_v1', '1')
    const { useMediaLibraryStore } = await import('../../store/medialibrary')
    const store = useMediaLibraryStore()
    await store.hydrate()
    const original = item('movie-a', '2026-07-01')
    store.addMediaItem(original)
    store.addToRecentlyAdded(original)

    store.replaceMediaItemMetadata({ ...original, name: 'Correct title', tmdbId: 123, metadataSource: 'manual' })

    expect(store.mediaItems).toHaveLength(1)
    expect(store.mediaItems[0].name).toBe('Correct title')
    expect(store.recentlyAdded).toHaveLength(1)
    expect(store.recentlyAdded[0].name).toBe('Correct title')
  })

  it('replaces metadata for a movie nested in a collection', async () => {
    localStorage.setItem('MediaLibrary_DexieMigrated_v1', '1')
    const { useMediaLibraryStore } = await import('../../store/medialibrary')
    const store = useMediaLibraryStore()
    await store.hydrate()
    const child = item('collection-movie', '2026-07-01')
    const collection = { ...item('collection', '2026-07-01'), collectionId: 10, collectionMovies: [child] }
    store.addMediaItem(collection)

    store.replaceMediaItemMetadata({ ...child, name: 'Correct collection movie', tmdbId: 456, metadataSource: 'manual' })

    expect(store.mediaItems).toHaveLength(1)
    expect(store.mediaItems[0].collectionMovies?.[0].name).toBe('Correct collection movie')
    expect(store.mediaItems[0].collectionMovies?.[0].tmdbId).toBe(456)
  })

  it('removes historical aliases of the same media source together', async () => {
    localStorage.setItem('MediaLibrary_DexieMigrated_v1', '1')
    const { useMediaLibraryStore } = await import('../../store/medialibrary')
    const store = useMediaLibraryStore()
    await store.hydrate()
    const source = { fileId: 'folder-1', name: 'Movies', path: '/Movies', userId: 'user', driveId: 'drive', driveServerId: 'aliyun', itemCount: 1, scanDate: new Date('2026-07-01') }
    store.addFolder({ ...source, id: 'legacy-source-id' })
    store.addFolder({ ...source, id: 'scoped-source-id' })
    store.addMediaItem({ ...item('movie-a', '2026-07-01'), folderId: 'legacy-source-id' })
    store.addMediaItem({ ...item('movie-b', '2026-07-01'), folderId: 'scoped-source-id' })

    store.removeFolder('scoped-source-id')

    expect(store.folders).toHaveLength(0)
    expect(store.mediaItems).toHaveLength(0)
  })
})
