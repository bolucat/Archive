import type { MediaLibraryItem, MediaEpisode } from '../types/media'
import { detailSeriesId } from './detailResume'

export function detailCollectionTarget(item: MediaLibraryItem, episode?: Pick<MediaEpisode, 'seasonNumber' | 'episodeNumber' | 'name'> | null) {
  if (item.type === 'tv') {
    if (!episode) return null
    return { id: `${detailSeriesId(item.id)}_${episode.seasonNumber}_${episode.episodeNumber}`, title: `${item.name} · S${episode.seasonNumber}E${episode.episodeNumber} ${episode.name}`.trim() }
  }
  return { id: item.id, title: item.name }
}

export function playlistSelection(playlists: Record<string, string[]>, itemId: string): string[] {
  return Object.keys(playlists).filter(name => playlists[name].includes(itemId))
}

export function applyPlaylistSelection(playlists: Record<string, string[]>, itemId: string, selected: string[]): Record<string, string[]> {
  if (!itemId) return playlists
  const included = new Set(selected)
  return Object.fromEntries(Object.entries(playlists).map(([name, ids]) => [name, included.has(name) ? [...new Set([...ids, itemId])] : ids.filter(id => id !== itemId)]))
}
