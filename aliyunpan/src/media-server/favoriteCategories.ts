import type { MediaServerConfig } from '../types/mediaServer'

export const SERVER_FAVORITE_TYPES = ['Series', 'Episode', 'Movie', 'Video', 'BoxSet', 'Playlist', 'Person'] as const
export type ServerFavoriteType = typeof SERVER_FAVORITE_TYPES[number]
export const serverFavoriteID = (type: ServerFavoriteType) => `server-favorites:${type}`
export function serverFavoriteType(id: string): ServerFavoriteType | undefined {
  if (!id.startsWith('server-favorites:')) return undefined
  return SERVER_FAVORITE_TYPES.find(type => serverFavoriteID(type) === id)
}
// Plex ratings are not Emby/Jellyfin favorite flags. Do not advertise unsupported queries.
export const serverFavoriteTypes = (server: MediaServerConfig) => server.type === 'plex' ? [] : SERVER_FAVORITE_TYPES
