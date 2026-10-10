import { describe, it, expect } from 'vitest'
import { isMediaWatched, setMediaWatched } from '../localWatchedState'
import type { MediaLibraryItem } from '../../types/media'
describe('poster watched actions', () => {
  const series = { id: 'series', driveFiles: [], seasons: [{ episodes: [{ seasonNumber: 1, episodeNumber: 1, driveFiles: [] }, { seasonNumber: 1, episodeNumber: 2, driveFiles: [] }] }] } as unknown as MediaLibraryItem
  it('does not treat one watched episode as the entire series', () => {
    expect(isMediaWatched(series, ['series_1_1'])).toBe(false)
    expect(isMediaWatched(series, ['series_1_1', 'series_1_2'])).toBe(true)
  })
  it('sets and clears canonical and all episode markers', () => {
    const state = { watchedItems: ['series_1_1'], markWatched(id: string, watched: boolean) { this.watchedItems = this.watchedItems.filter(key => key !== id); if (watched) this.watchedItems.push(id) } }
    setMediaWatched(series, true, state)
    expect(isMediaWatched(series, state.watchedItems)).toBe(true)
    setMediaWatched(series, false, state)
    expect(state.watchedItems).toEqual([])
    expect(isMediaWatched(series, state.watchedItems)).toBe(false)
  })
})
