import { describe, expect, it, vi } from 'vitest'
import { buildMediaOrganizePlan, executeMediaOrganizePlan } from '../drive-tools/mediaOrganize'

vi.mock('../drive-tools/directLinks', () => ({
  listDriveToolChildren: vi.fn(async () => [])
}))

vi.mock('../../aliapi/filecmd', () => ({
  default: {
    ApiCreatNewForder: vi.fn(async (_userId: string, _driveId: string, parentId: string, name: string) => ({ file_id: `${parentId}/${name}` })),
    ApiMoveBatch: vi.fn(async (_userId: string, _driveId: string, fileIds: string[]) => fileIds)
  }
}))

vi.mock('../webdavClient', () => ({
  createWebDavDirectory: vi.fn(async () => undefined),
  getWebDavConnection: vi.fn(() => ({ id: 'dav' })),
  getWebDavConnectionId: vi.fn(() => 'dav'),
  moveWebDavPath: vi.fn(async () => undefined),
  normalizeWebDavPath: (value: string) => {
    const trimmed = String(value || '/').replace(/^\/+|\/+$/g, '')
    return trimmed ? `/${trimmed}` : '/'
  }
}))

describe('drive-tools media organize', () => {
  it('builds helper-style preview paths from media names', () => {
    const result = buildMediaOrganizePlan([
      { userId: 'u', driveId: 'guangya', fileId: '1', name: '测试剧 S02E01 2025.mp4', isDir: false }
    ], 'root')
    expect(result[0].category).toBe('电视剧')
    expect(result[0].targetPath).toContain('电视剧')
    expect(result[0].targetPath).toContain('Season 02')
  })

  it('executes media organize for WebDAV items', async () => {
    await expect(executeMediaOrganizePlan([
      {
        userId: 'dav',
        driveId: 'webdav:dav',
        fileId: '/downloads/测试剧 S02E01 2025.mp4',
        name: '测试剧 S02E01 2025.mp4',
        isDir: false,
        category: '电视剧',
        targetSegments: ['电视剧', '测试剧 (2025)', 'Season 02'],
        targetPath: '/downloads / 电视剧 / 测试剧 (2025) / Season 02',
        title: '测试剧',
        year: '2025',
        season: 2
      }
    ], '/downloads')).resolves.toMatchObject({
      total: 1,
      success: 1,
      failed: 0
    })
  })
})
