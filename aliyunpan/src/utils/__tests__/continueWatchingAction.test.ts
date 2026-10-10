import { afterEach, describe, expect, it, vi } from 'vitest'
import { isContinueWatchingMember, toggleContinueWatching, watchingUpdatePending, cancelWatchingUpdate, WATCHING_INDICATOR_MIN_MS } from '../continueWatchingAction'
import type { MediaLibraryItem } from '../../types/media'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { parse, compileScript, compileTemplate } from 'vue/compiler-sfc'

const movie: MediaLibraryItem = { id: 'movie_1', name: 'Movie', type: 'movie', genres: [], driveFiles: [], addedAt: new Date(), parentId: 'root' }
function setup(items: MediaLibraryItem[] = []) {
  vi.stubGlobal('window', {})
  vi.stubGlobal('requestAnimationFrame', (fn: () => void) => { queueMicrotask(fn); return 1 })
  const setItem = vi.fn()
  vi.stubGlobal('localStorage', { setItem })
  const store = { continueWatching: items, addToContinueWatching(item: MediaLibraryItem) { this.continueWatching = [item, ...this.continueWatching.filter(entry => entry.id !== item.id)] } }
  return { store, setItem }
}
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals() })
describe('continue watching menu actions', () => {
  it('keeps the loading indication visible for fast local operations', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'performance'] })
    const { store } = setup()
    const operation = toggleContinueWatching(store, movie)
    await vi.advanceTimersByTimeAsync(0)
    expect(store.continueWatching).toHaveLength(1)
    expect(watchingUpdatePending.value).toBe(true)
    await vi.advanceTimersByTimeAsync(WATCHING_INDICATOR_MIN_MS - 1)
    expect(watchingUpdatePending.value).toBe(true)
    await vi.advanceTimersByTimeAsync(1)
    await operation
    expect(watchingUpdatePending.value).toBe(false)
  })
  it('does not animate the full-screen mask or scale the loading dialog', () => {
    const source = readFileSync('src/components/WatchingUpdateModal.vue', 'utf8')
    expect(source).toContain('mask-animation-name="" modal-animation-name=""')
    expect(source).toContain('role="status"')
    expect(source).toContain("backdropFilter: 'none'")
  })
  it('adds, persists and removes membership and exposes pending state', async () => {
    const { store, setItem } = setup()
    const pending = toggleContinueWatching(store, movie)
    expect(watchingUpdatePending.value).toBe(true)
    expect(store.continueWatching).toEqual([])
    await pending
    expect(watchingUpdatePending.value).toBe(false)
    expect(store.continueWatching.map(item => item.id)).toEqual([movie.id])
    expect(setItem).toHaveBeenCalledWith('MediaLibrary_ContinueWatching', expect.any(String))
    await toggleContinueWatching(store, movie)
    expect(store.continueWatching).toEqual([])
  })
  it('cancels without changing membership or storage', async () => {
    const { store, setItem } = setup()
    const pending = toggleContinueWatching(store, movie)
    cancelWatchingUpdate()
    await pending
    expect(store.continueWatching).toEqual([])
    expect(setItem).not.toHaveBeenCalled()
  })
  it('rolls back on persistence failure', async () => {
    const { store, setItem } = setup()
    setItem.mockImplementation(() => { throw new Error('Storage full') })
    await expect(toggleContinueWatching(store, movie)).rejects.toThrow('Storage full')
    expect(store.continueWatching).toEqual([])
    expect(watchingUpdatePending.value).toBe(false)
  })
  it('syncs add/remove when connected, then persists the resulting local membership', async () => {
    const { store } = setup()
    const invoke = vi.fn(async (channel: string) => channel === 'trakt:status' ? { configured: true, connected: true } : undefined)
    vi.stubGlobal('window', { Electron: { ipcRenderer: { invoke } } })
    const target = { ...movie, tmdbId: 123 }
    await toggleContinueWatching(store, target)
    expect(invoke).toHaveBeenCalledWith('trakt:sync', { type: 'movie', tmdbId: 123 }, 'watchlist-add', undefined)
    expect(store.continueWatching).toHaveLength(1)
    await toggleContinueWatching(store, target)
    expect(invoke).toHaveBeenCalledWith('trakt:sync', { type: 'movie', tmdbId: 123 }, 'watchlist-remove', undefined)
    expect(store.continueWatching).toHaveLength(0)
  })
  it('does not flip the menu membership or persist when remote sync fails', async () => {
    const { store, setItem } = setup()
    const invoke = vi.fn(async (channel: string) => {
      if (channel === 'trakt:status') return { configured: true, connected: true }
      throw new Error('TRAKT_NOT_FOUND')
    })
    vi.stubGlobal('window', { Electron: { ipcRenderer: { invoke } } })
    await expect(toggleContinueWatching(store, movie)).rejects.toThrow()
    expect(store.continueWatching).toEqual([])
    expect(setItem).not.toHaveBeenCalled()
    expect(watchingUpdatePending.value).toBe(false)
  })
  it('does not hide manually re-added completed titles from the homepage', async () => {
    const { store } = setup()
    await toggleContinueWatching(store, { ...movie, watchProgress: 100 })
    expect(store.continueWatching[0].watchProgress).toBe(0)
  })
  it('matches canonical TV episode IDs without mixing different series or episodes', () => {
    const tv = { id: 'tv_12', type: 'tv' as const }
    expect(isContinueWatchingMember({ id: 'tv_12_2_3' }, tv)).toBe(true)
    expect(isContinueWatchingMember({ id: 'tv_123_2_3' }, tv)).toBe(false)
    expect(isContinueWatchingMember({ id: 'tv_12_2_4' }, { ...tv, id: 'tv_12_2_3' })).toBe(false)
  })
  it('removes saved episode entries when removing a series', async () => {
    const { store } = setup([{ ...movie, type: 'tv', id: 'tv_12_2_3' }, movie])
    await toggleContinueWatching(store, { ...movie, type: 'tv', id: 'tv_12' })
    expect(store.continueWatching).toEqual([movie])
  })
  it.each(['MediaPosterMenu.vue', 'WatchingUpdateModal.vue', 'TraktAccountModal.vue', 'TraktAvatar.vue', 'MediaPersonalRatingModal.vue', 'MediaLibrary.vue', 'MediaPanRight.vue', 'UnifiedMediaRow.vue'])('compiles %s', path => {
    const source = readFileSync(resolve(process.cwd(), 'src/components', path), 'utf8')
    const { descriptor } = parse(source)
    const script = compileScript(descriptor, { id: path })
    expect(compileTemplate({ source: descriptor.template!.content, filename: path, id: path, compilerOptions: { bindingMetadata: script.bindings } }).errors).toEqual([])
  })
})
