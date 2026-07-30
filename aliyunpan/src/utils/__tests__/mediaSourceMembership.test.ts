import { describe, expect, it } from 'vitest'
import type { DriveFileItem, MediaLibraryItem } from '../../types/media'
import { mergeDriveFileSources, reconcileMediaItemSource } from '../mediaSourceMembership'

const file = (id: string, sources: string[]): DriveFileItem => ({
  id,
  name: `${id}.mkv`,
  path: `/${id}.mkv`,
  driveId: 'drive',
  driveServerId: 'provider',
  userId: 'user',
  fileSize: 1,
  sourceFolderIds: sources
})

const movie = (driveFiles: DriveFileItem[]): MediaLibraryItem => ({
  id: 'movie',
  parentId: '',
  folderId: 'root-source',
  type: 'movie',
  name: 'Movie',
  genres: [],
  driveFiles,
  addedAt: new Date()
})

describe('media source membership', () => {
  it('merges overlapping source membership for the same physical file', () => {
    const merged = mergeDriveFileSources([file('same', ['root-source']), file('same', ['child-source'])])
    expect(merged).toHaveLength(1)
    expect(merged[0].sourceFolderIds?.sort()).toEqual(['child-source', 'root-source'])
  })

  it('removes only the stale source and keeps a file owned by another source', () => {
    const result = reconcileMediaItemSource(movie([file('same', ['root-source', 'child-source'])]), 'child-source', new Set())
    expect(result.item?.driveFiles).toHaveLength(1)
    expect(result.item?.driveFiles[0].sourceFolderIds).toEqual(['root-source'])
  })

  it('removes the media item after its final source disappears', () => {
    const result = reconcileMediaItemSource(movie([file('same', ['root-source'])]), 'root-source', new Set())
    expect(result.item).toBeUndefined()
  })

  it('keeps files seen during a completed rescan', () => {
    const item = movie([file('same', ['root-source'])])
    const result = reconcileMediaItemSource(item, 'root-source', new Set(['provider:user:drive:same']))
    expect(result).toEqual({ item, changed: false })
  })
})
