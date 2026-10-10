import { ref, watch } from 'vue'

export type LibraryBrowseMode = 'grid' | 'list'
export const libraryBrowseModeKey = 'MediaLibrary_BrowseMode'

export function readLibraryBrowseMode(storage: Pick<Storage, 'getItem'>): LibraryBrowseMode {
  try { return storage.getItem(libraryBrowseModeKey) === 'list' ? 'list' : 'grid' } catch { return 'grid' }
}

const browseMode = ref<LibraryBrowseMode>(typeof localStorage === 'undefined' ? 'grid' : readLibraryBrowseMode(localStorage))
watch(browseMode, mode => {
  try { localStorage.setItem(libraryBrowseModeKey, mode) } catch { /* Keep the current session usable if storage is unavailable. */ }
}, { flush: 'sync' })

/** Shared by home See All, sidebar categories, search, and the standalone library. */
export function useLibraryBrowseMode() { return browseMode }
