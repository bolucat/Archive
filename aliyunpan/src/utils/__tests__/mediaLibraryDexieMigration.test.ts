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
})
