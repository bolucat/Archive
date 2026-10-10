import { nextTick, ref } from 'vue'
import type { MediaLibraryItem } from '../types/media'
import { loadTraktStatus, syncTrakt, traktError } from '../services/trakt/client'

export function isContinueWatchingMember(entry: Pick<MediaLibraryItem, 'id'>, target: Pick<MediaLibraryItem, 'id' | 'type'>): boolean {
  if (entry.id === target.id) return true
  return target.type === 'tv' && !/_\d+_\d+$/.test(target.id) && entry.id.startsWith(target.id + '_') && /^\d+_\d+$/.test(entry.id.slice(target.id.length + 1))
}

export const watchingUpdatePending = ref(false)
export const WATCHING_INDICATOR_MIN_MS = 450
let cancelled = false
export function cancelWatchingUpdate() { cancelled = true }

export async function toggleContinueWatching(store: { continueWatching: MediaLibraryItem[]; addToContinueWatching: (item: MediaLibraryItem) => void }, target: MediaLibraryItem) {
  if (watchingUpdatePending.value) return
  watchingUpdatePending.value = true
  const startedAt = performance.now()
  cancelled = false
  const before = [...store.continueWatching]
  try {
    await nextTick()
    // Let the loading dialog paint before serializing and persisting the list.
    await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))
    if (cancelled) return
    const removing = store.continueWatching.some(entry => isContinueWatchingMember(entry, target))
    // No IPC in browser-only mode; a desktop status failure must not silently skip sync.
    if (typeof window !== 'undefined' && window.Electron?.ipcRenderer && (await loadTraktStatus()).connected) {
      if (cancelled) return
      await syncTrakt(target as import('@shared/trakt').TraktIdentity, removing ? 'watchlist-remove' : 'watchlist-add')
      // Once submitted, finish local persistence even if cancellation was requested.
    } else if (cancelled) return
    if (removing) {
      store.continueWatching = store.continueWatching.filter(entry => !isContinueWatchingMember(entry, target))
    } else {
      const progress = Number(target.watchProgress) || 0
      store.addToContinueWatching({ ...target, watchProgress: progress >= 100 ? 0 : Math.max(0, progress) })
    }
    localStorage.setItem('MediaLibrary_ContinueWatching', JSON.stringify(store.continueWatching))
    await nextTick()
  } catch (error) {
    store.continueWatching = before
    if (String(error).includes('TRAKT_')) throw new Error(traktError(error))
    throw error
  } finally {
    // A fast local update must not open and close the screen mask in a single frame.
    // Network operations are never delayed before submission; only the indicator's
    // minimum visible duration is bounded. Explicit cancellation closes promptly.
    const remaining = WATCHING_INDICATOR_MIN_MS - (performance.now() - startedAt)
    if (!cancelled && remaining > 0) await new Promise<void>(resolve => setTimeout(resolve, remaining))
    watchingUpdatePending.value = false
  }
}
