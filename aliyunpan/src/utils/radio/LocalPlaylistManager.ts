/**
 * LocalPlaylistManager — create, save, load, import/export local playlists.
 * Playlists are stored in localStorage as JSON. M3U import/export supported.
 */

import type { IPageMusicTrack } from '../../store/appstore'

export interface LocalPlaylist {
  id: string
  name: string
  tracks: IPageMusicTrack[]
  createdAt: number
  updatedAt: number
}

const STORE_KEY = 'boxplayer-local-playlists'

function genId(): string {
  return 'pl_' + Date.now().toString(36) + '_' + Math.random().toString(36).slice(2, 8)
}

export function loadPlaylists(): LocalPlaylist[] {
  try {
    const raw = localStorage.getItem(STORE_KEY)
    return raw ? JSON.parse(raw) : []
  } catch {
    return []
  }
}

export function savePlaylists(playlists: LocalPlaylist[]) {
  localStorage.setItem(STORE_KEY, JSON.stringify(playlists))
}

export function createPlaylist(name: string, tracks: IPageMusicTrack[] = []): LocalPlaylist {
  const now = Date.now()
  return { id: genId(), name, tracks: [...tracks], createdAt: now, updatedAt: now }
}

export function addTracksToList(list: LocalPlaylist, tracks: IPageMusicTrack[]): LocalPlaylist {
  const existingIds = new Set(list.tracks.map(t => t.file_id))
  const newTracks = tracks.filter(t => !existingIds.has(t.file_id))
  return { ...list, tracks: [...list.tracks, ...newTracks], updatedAt: Date.now() }
}

export function removeTrackFromList(list: LocalPlaylist, fileId: string): LocalPlaylist {
  return { ...list, tracks: list.tracks.filter(t => t.file_id !== fileId), updatedAt: Date.now() }
}

export function renamePlaylist(list: LocalPlaylist, name: string): LocalPlaylist {
  return { ...list, name, updatedAt: Date.now() }
}

/** Parse M3U content into a list of file paths (for matching to cloud tracks). */
export function parseM3U(content: string): string[] {
  const lines = content.split(/\r?\n/)
  const paths: string[] = []
  for (const line of lines) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue
    paths.push(trimmed)
  }
  return paths
}

/** Export a playlist as M3U content string. */
export function exportM3U(list: LocalPlaylist): string {
  let out = '#EXTM3U\n'
  out += `#PLAYLIST:${list.name}\n`
  out += `#CREATED:${new Date(list.createdAt).toISOString()}\n`
  for (const t of list.tracks) {
    out += `#EXTINF:-1,${t.file_name}\n`
    out += `${t.file_name}\n`
  }
  return out
}
