import type { MediaServerConfig } from '../types/mediaServer'
import { mediaServerFetchVoid } from './http'

/** Deletes the selected item and its media through the owning server only. */
export async function deleteMediaServerItem(server: MediaServerConfig, itemId: string): Promise<void> {
  if (!itemId.trim()) throw new Error('Missing media item ID')
  if (!['emby', 'jellyfin', 'plex'].includes(server.type)) throw new Error('Unsupported media server')
  const id = encodeURIComponent(itemId)
  const path = server.type === 'plex' ? '/library/metadata/' + id : '/Items/' + id
  await mediaServerFetchVoid(server, path, { method: 'DELETE' })
}
