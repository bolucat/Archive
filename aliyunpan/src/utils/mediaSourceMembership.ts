import type { DriveFileItem, MediaLibraryItem } from '../types/media'

export function mediaDriveFileKey(file: Pick<DriveFileItem, 'driveServerId' | 'userId' | 'driveId' | 'id'>): string {
  return [file.driveServerId, file.userId, file.driveId, file.id].map(value => encodeURIComponent(String(value || ''))).join(':')
}

export function mergeDriveFileSources<T extends DriveFileItem>(files: T[]): T[] {
  const merged = new Map<string, T>()
  for (const file of files) {
    const key = mediaDriveFileKey(file)
    const existing = merged.get(key)
    if (!existing) {
      merged.set(key, file)
      continue
    }
    merged.set(key, {
      ...existing,
      ...file,
      sourceFolderIds: Array.from(new Set([...(existing.sourceFolderIds || []), ...(file.sourceFolderIds || [])]))
    })
  }
  return [...merged.values()]
}

export function reconcileMediaItemSource(item: MediaLibraryItem, folderId: string, seenFileKeys: Set<string>): { item?: MediaLibraryItem; changed: boolean } {
  let changed = false
  const reconcileFiles = (files: DriveFileItem[] = []) => files.flatMap((file) => {
    const sources = file.sourceFolderIds?.length ? file.sourceFolderIds : (item.folderId === folderId ? [folderId] : [])
    if (!sources.includes(folderId) || seenFileKeys.has(mediaDriveFileKey(file))) return [file]
    changed = true
    const nextSources = sources.filter(source => source !== folderId)
    return nextSources.length ? [{ ...file, sourceFolderIds: nextSources }] : []
  })

  const driveFiles = reconcileFiles(item.driveFiles)
  const seasons = item.seasons?.map(season => ({
    ...season,
    episodes: season.episodes?.map(episode => ({ ...episode, driveFiles: reconcileFiles(episode.driveFiles) })).filter(episode => episode.driveFiles.length)
  })).filter(season => season.episodes?.length)
  const collectionMovies = item.collectionMovies?.map(movie => ({ ...movie, driveFiles: reconcileFiles(movie.driveFiles) })).filter(movie => movie.driveFiles.length)
  const hasFiles = driveFiles.length || seasons?.some(season => season.episodes?.some(episode => episode.driveFiles.length)) || collectionMovies?.some(movie => movie.driveFiles.length)
  if (!hasFiles) return { changed: true }
  if (!changed) return { item, changed: false }

  const remainingSource = driveFiles[0]?.sourceFolderIds?.[0]
    || seasons?.flatMap(season => season.episodes || []).flatMap(episode => episode.driveFiles)[0]?.sourceFolderIds?.[0]
    || collectionMovies?.flatMap(movie => movie.driveFiles)[0]?.sourceFolderIds?.[0]
  return {
    changed: true,
    item: {
      ...item,
      driveFiles,
      seasons,
      collectionMovies,
      folderId: item.folderId === folderId ? remainingSource : item.folderId
    }
  }
}
