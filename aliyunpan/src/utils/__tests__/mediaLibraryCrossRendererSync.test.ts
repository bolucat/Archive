import { beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { syncMediaLibraryStoreFromStorage, useMediaLibraryStore } from '../../store/medialibrary'

class MemoryStorage implements Storage {
  private values = new Map<string, string>()

  get length() { return this.values.size }
  clear() { this.values.clear() }
  getItem(key: string) { return this.values.get(key) ?? null }
  key(index: number) { return [...this.values.keys()][index] ?? null }
  removeItem(key: string) { this.values.delete(key) }
  setItem(key: string, value: string) { this.values.set(key, value) }
}

describe('media library cross-renderer synchronization', () => {
  beforeEach(() => {
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: new MemoryStorage() })
    setActivePinia(createPinia())
  })

  it('reloads newly scraped media and folders without restarting the app', () => {
    const store = useMediaLibraryStore()
    expect(store.mediaItems).toHaveLength(0)
    expect(store.folders).toHaveLength(0)

    localStorage.setItem('MediaLibrary_MediaItems', JSON.stringify([{
      id: 'quark-movie',
      type: 'movie',
      name: '千与千寻',
      genres: ['动画'],
      driveFiles: [],
      addedAt: '2026-07-21T14:00:00.000Z'
    }]))
    localStorage.setItem('MediaLibrary_Folders', JSON.stringify([{
      id: 'quark-folder',
      fileId: 'folder-1',
      name: '千与千寻',
      userId: 'quark-user',
      driveId: 'quark',
      driveServerId: 'quark',
      scanDate: '2026-07-21T14:00:00.000Z',
      itemCount: 1
    }]))

    // HMR can retain the store instance that existed before a newly-added
    // action was registered. Cross-renderer sync must not depend on that action.
    delete (store as any).reloadLibraryFromStorage
    syncMediaLibraryStoreFromStorage(store)

    expect(store.mediaItems.map(item => item.name)).toEqual(['千与千寻'])
    expect(store.mediaItems[0].addedAt).toBeInstanceOf(Date)
    expect(store.folders.map(folder => folder.name)).toEqual(['千与千寻'])
    expect(store.folders[0].scanDate).toBeInstanceOf(Date)
  })
})
