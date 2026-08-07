import { describe, expect, it, vi } from 'vitest'
import { deleteDriveEmptyDirs, scanDriveEmptyDirs } from '../drive-tools/emptyDirs'

vi.mock('../../aliapi/filecmd', () => ({
  default: {
    ApiTrashBatch: vi.fn(async (_userId: string, _driveId: string, fileIds: string[]) => fileIds.slice(0, 1))
  }
}))

vi.mock('../webdavClient', () => ({
  deleteWebDavPath: vi.fn(async () => undefined),
  getWebDavConnection: vi.fn(() => ({ id: 'dav' })),
  getWebDavConnectionId: vi.fn(() => 'dav')
}))

vi.mock('../drive-tools/directLinks', () => ({
  listDriveToolChildren: vi.fn(async (_userId: string, _driveId: string, fileId: string) => {
    if (fileId === 'root') return [{ file_id: 'denied', parent_file_id: 'root', drive_id: 'guangya', name: 'denied', isDir: true }]
    throw new Error('permission denied')
  })
}))

describe('drive-tools empty dirs', () => {
  it('does not call provider api for empty delete input', async () => {
    const result = await deleteDriveEmptyDirs([])
    expect(result).toEqual({
      total: 0,
      success: 0,
      failed: 0,
      deletedFileKeys: [],
      report: '没有可删除的空目录'
    })
  })

  it('returns deleted keys for partial provider success', async () => {
    const result = await deleteDriveEmptyDirs([
      { userId: 'u', driveId: 'guangya', fileId: 'a', parentFileId: 'root', name: 'a', path: 'a' },
      { userId: 'u', driveId: 'guangya', fileId: 'b', parentFileId: 'root', name: 'b', path: 'b' }
    ])
    expect(result).toMatchObject({ total: 2, success: 1, failed: 1 })
    expect(result.deletedFileKeys).toEqual(['u\nguangya\na'])
  })

  it('keeps WebDAV empty directories read-only', async () => {
    const result = await deleteDriveEmptyDirs([
      { userId: 'dav', driveId: 'webdav:dav', fileId: '/empty', parentFileId: '/', name: 'empty', path: 'empty' }
    ])
    expect(result).toMatchObject({ total: 1, success: 0, failed: 1 })
    expect(result.deletedFileKeys).toEqual([])
  })

  it('does not classify an unreadable child directory as empty', async () => {
    const result = await scanDriveEmptyDirs('u', 'guangya', 'root')
    expect(result).toMatchObject({ emptyDirs: [], failedDirs: 1, errors: ['denied：permission denied'] })
  })
})
