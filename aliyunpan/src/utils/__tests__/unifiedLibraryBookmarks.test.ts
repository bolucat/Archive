import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import useBookmarks from '../../store/unifiedLibraryBookmarks'

const cache = new Map<string, string>()
beforeEach(() => {
  cache.clear()
  setActivePinia(createPinia())
  vi.stubGlobal('localStorage', { getItem: (key: string) => cache.get(key) || null, setItem: (key: string, value: string) => cache.set(key, value) })
})
afterEach(() => vi.unstubAllGlobals())
describe('category bookmarks', () => {
  it('includes books in default favorites without putting it on home implicitly', () => {
    const store = useBookmarks()
    expect(store.favorites).toContain('books')
    expect(store.home).toEqual([])
  })
  it('persists independent home and favorite membership across sessions', () => {
    const store = useBookmarks()
    store.toggle('favorites', 'movies/genres/Drama')
    store.toggle('home', 'movies/genres/Drama')
    store.toggle('favorites', 'movies/genres/Drama')
    setActivePinia(createPinia())
    const restored = useBookmarks()
    restored.ensureLoaded()
    expect(restored.favorites).not.toContain('movies/genres/Drama')
    expect(restored.home).toEqual(['movies/genres/Drama'])
  })
  it('retains explicit removal of every favorite without restoring defaults', () => {
    const store = useBookmarks()
    for (const id of [...store.favorites]) store.toggle('favorites', id)
    setActivePinia(createPinia())
    const restored = useBookmarks()
    restored.ensureLoaded()
    expect(restored.favorites).toEqual([])
  })
})
