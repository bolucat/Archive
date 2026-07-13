import { describe, expect, it, vi } from 'vitest'
import { deleteDriveEmptyDirs } from '../drive-tools/emptyDirs'

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

  it('deletes WebDAV empty directories directly', async () => {
    const result = await deleteDriveEmptyDirs([
      { userId: 'dav', driveId: 'webdav:dav', fileId: '/empty', parentFileId: '/', name: 'empty', path: 'empty' }
    ])
    expect(result).toMatchObject({ total: 1, success: 1, failed: 0 })
    expect(result.deletedFileKeys).toEqual(['dav\nwebdav:dav\n/empty'])
  })
})
