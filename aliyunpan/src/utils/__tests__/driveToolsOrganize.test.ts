import { describe, expect, it, vi } from 'vitest'
import { moveDriveToolFiles } from '../drive-tools/organize'

vi.mock('../../aliapi/filecmd', () => ({
  default: {
    ApiMoveBatch: vi.fn(async (_userId: string, _driveId: string, fileIds: string[]) => fileIds)
  }
}))

vi.mock('../webdavClient', () => ({
  getWebDavConnection: vi.fn(() => ({ id: 'dav' })),
  getWebDavConnectionId: vi.fn(() => 'dav'),
  moveWebDavPath: vi.fn(async () => undefined),
  normalizeWebDavPath: (value: string) => {
    const trimmed = String(value || '/').replace(/^\/+|\/+$/g, '')
    return trimmed ? `/${trimmed}` : '/'
  }
}))

describe('drive-tools organize', () => {
  it('does not call a provider when no files or target is supplied', async () => {
    await expect(moveDriveToolFiles([], '')).resolves.toEqual({
      total: 0,
      success: 0,
      failed: 0,
      report: '没有可移动的文件或目标目录'
    })
  })

  it('treats ApiMoveBatch return value as successful file ids', async () => {
    await expect(moveDriveToolFiles([
      { userId: 'u', driveId: 'drive115', fileId: 'f1', name: 'a.mkv' },
      { userId: 'u', driveId: 'drive115', fileId: 'f2', name: 'b.mkv' }
    ], 'target')).resolves.toMatchObject({
      total: 2,
      success: 2,
      failed: 0
    })
  })

  it('moves WebDAV items without calling provider move api', async () => {
    await expect(moveDriveToolFiles([
      { userId: 'dav', driveId: 'webdav:dav', fileId: '/old/a.mkv', name: 'a.mkv' }
    ], '/target', 'webdav:dav')).resolves.toMatchObject({
      total: 1,
      success: 1,
      failed: 0
    })
  })

  it('does not move WebDAV items across drives', async () => {
    await expect(moveDriveToolFiles([
      { userId: 'dav', driveId: 'webdav:dav', fileId: '/old/a.mkv', name: 'a.mkv' }
    ], '/target', 'webdav:other')).resolves.toMatchObject({
      total: 1,
      success: 0,
      failed: 1
    })
  })
})
