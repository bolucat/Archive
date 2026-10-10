import type { MediaLibraryItem } from '../types/media'
export const localWatchedKey = (path: string) => 'local-file:' + path
type WatchedMedia = Pick<MediaLibraryItem, 'id' | 'driveFiles'> & Partial<Pick<MediaLibraryItem, 'seasons'>>
export function localWatchedKeys(item: WatchedMedia): string[] {
  return [...(item.driveFiles || []), ...(item.seasons || []).flatMap(season => (season.episodes || []).flatMap(episode => episode.driveFiles || []))]
    .filter(file => file.driveId === 'local' || file.driveServerId === 'local')
    .map(file => localWatchedKey(file.path || file.id))
}
export function isMediaWatched(item: WatchedMedia, watched: string[]): boolean {
  if (watched.includes(item.id)) return true
  const episodes = (item.seasons || []).flatMap(season => season.episodes || [])
  if (episodes.length) return episodes.every(episode => watched.includes(item.id + '_' + episode.seasonNumber + '_' + episode.episodeNumber) || (episode.driveFiles.length > 0 && episode.driveFiles.every(file => watched.includes(localWatchedKey(file.path || file.id)))))
  return localWatchedKeys(item).some(key => watched.includes(key))
}

export function setMediaWatched(item: WatchedMedia, watched: boolean, state: { watchedItems: string[]; markWatched: (id: string, watched: boolean) => void }): void {
  state.markWatched(item.id, watched)
  for (const key of localWatchedKeys(item)) state.markWatched(key, watched)
  for (const episode of (item.seasons || []).flatMap(season => season.episodes || [])) state.markWatched(item.id + '_' + episode.seasonNumber + '_' + episode.episodeNumber, watched)
}
