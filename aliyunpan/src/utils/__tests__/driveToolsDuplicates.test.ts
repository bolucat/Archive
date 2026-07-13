import { describe, expect, it, vi } from 'vitest'
import { scanDriveDuplicates } from '../drive-tools/duplicates'

vi.mock('../drive-tools/directLinks', () => ({
  listDriveToolChildren: vi.fn(async (_userId: string, _driveId: string, fileId: string) => {
    if (fileId === 'root') return [
      { __v_skip: true, drive_id: 'guangya', file_id: 'dir', parent_file_id: 'root', name: '电影', isDir: true, size: 0, sizeStr: '', time: 0, timeStr: '', icon: 'folder' },
      { __v_skip: true, drive_id: 'guangya', file_id: 'plain', parent_file_id: 'root', name: 'movie.mp4', isDir: false, size: 10, sizeStr: '10 B', time: 1, timeStr: '', icon: 'file', content_hash: 'hash-a' }
    ]
    return [
      { __v_skip: true, drive_id: 'guangya', file_id: 'one', parent_file_id: 'dir', name: 'movie(1).mp4', isDir: false, size: 10, sizeStr: '10 B', time: 1, timeStr: '', icon: 'file', content_hash: 'hash-a' },
      { __v_skip: true, drive_id: 'guangya', file_id: 'two', parent_file_id: 'dir', name: 'movie（２）.mp4', isDir: false, size: 10, sizeStr: '10 B', time: 2, timeStr: '', icon: 'file', content_hash: 'hash-a' }
    ]
  })
}))

describe('drive-tools duplicates', () => {
  const target = { userId: 'u', driveId: 'guangya', rootId: 'root', name: '光鸭' }

  it('matches helper suffix duplicates across nested directories', async () => {
    const result = await scanDriveDuplicates([target], 'helperName')
    expect(result.groups).toHaveLength(1)
    expect(result.groups[0].files.map(file => file.name)).toEqual(['movie(1).mp4', 'movie（２）.mp4'])
  })

  it('groups content hashes only when the same hash appears twice', async () => {
    const result = await scanDriveDuplicates([target], 'contentHash')
    expect(result.groups).toHaveLength(1)
    expect(result.groups[0].files).toHaveLength(3)
  })
})
