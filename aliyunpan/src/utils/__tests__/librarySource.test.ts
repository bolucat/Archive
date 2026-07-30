import { describe, expect, it } from 'vitest'
import { buildLibrarySourceId } from '../../types/librarySource'

describe('library source identity', () => {
  it('separates media kind, account, drive and folder', () => {
    expect(buildLibrarySourceId('music', 'user-a', 'drive-a', 'folder-a')).toBe('music|user-a|drive-a|folder-a')
    expect(buildLibrarySourceId('book', 'user-a', 'drive-a', 'folder-a')).not.toBe(buildLibrarySourceId('music', 'user-a', 'drive-a', 'folder-a'))
    expect(buildLibrarySourceId('music', 'user-b', 'drive-a', 'folder-a')).not.toBe(buildLibrarySourceId('music', 'user-a', 'drive-a', 'folder-a'))
    expect(buildLibrarySourceId('music', 'user-a', 'drive-b', 'folder-a')).not.toBe(buildLibrarySourceId('music', 'user-a', 'drive-a', 'folder-a'))
    expect(buildLibrarySourceId('music', 'user-a', 'drive-a', 'folder-b')).not.toBe(buildLibrarySourceId('music', 'user-a', 'drive-a', 'folder-a'))
  })
})
