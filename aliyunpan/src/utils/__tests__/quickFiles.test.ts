import { describe, expect, it } from 'vitest'
import { buildQuickFilePath, mergeQuickFiles, migrateLegacyQuickFiles, quickFileId, type QuickFileEntry } from '../../pan/quickFiles'

const entry = (user_id: string, drive_id: string, file_id: string): QuickFileEntry => ({
  id: quickFileId(user_id, drive_id, file_id),
  user_id,
  user_name: user_id,
  provider: 'test',
  drive_id,
  drive_name: drive_id,
  file_id,
  parent_file_id: 'root',
  path: `/${file_id}`,
  title: file_id,
  dir_path: []
})

describe('multi-drive quick files', () => {
  it('keeps identical provider file ids from different accounts and drives', () => {
    const merged = mergeQuickFiles([], [entry('user-a', 'drive-a', 'same-id'), entry('user-b', 'drive-b', 'same-id')])
    expect(merged).toHaveLength(2)
    expect(new Set(merged.map(item => item.id)).size).toBe(2)
  })

  it('updates only the matching composite shortcut', () => {
    const first = entry('user-a', 'drive-a', 'same-id')
    const second = entry('user-b', 'drive-b', 'same-id')
    const merged = mergeQuickFiles([first, second], [{ ...first, title: 'Renamed' }])
    expect(merged.map(item => item.title)).toEqual(['Renamed', 'same-id'])
  })

  it('migrates legacy per-user records into versioned entries', () => {
    const migrated = migrateLegacyQuickFiles([], 'legacy-user', [{ key: '/Movies', drive_id: 'webdav:home', drive_name: 'Home', title: 'Movies' }], 'webdav', 'Legacy Account')
    expect(migrated).toEqual([
      expect.objectContaining({
        id: quickFileId('legacy-user', 'webdav:home', '/Movies'),
        user_id: 'legacy-user',
        user_name: 'Legacy Account',
        provider: 'webdav',
        file_id: '/Movies',
        path: '/Movies'
      })
    ])
  })

  it('restores the target directory when an old shortcut has no cached path', () => {
    const item = entry('user-a', 'webdav:home', '/Movies/Anime')
    item.dir_path = []
    expect(buildQuickFilePath(item)).toEqual([
      expect.objectContaining({
        drive_id: 'webdav:home',
        file_id: '/Movies/Anime',
        name: '/Movies/Anime'
      })
    ])
  })
})
