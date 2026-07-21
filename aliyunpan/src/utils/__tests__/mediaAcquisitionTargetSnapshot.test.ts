import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { MediaAcquisitionTarget } from '@shared/types/mediaAcquisition'

const { apiDrive115FileList } = vi.hoisted(() => ({
  apiDrive115FileList: vi.fn()
}))

vi.mock('../../cloud115/dirfilelist', () => ({ apiDrive115FileList }))
vi.mock('../../aliapi/trash', () => ({ default: { ApiDirFileListNoLock: vi.fn() } }))
vi.mock('../../cloud123/dirfilelist', () => ({ apiCloud123FileListPage: vi.fn() }))
vi.mock('../../guangya/dirfilelist', () => ({ apiGuangyaFileList: vi.fn(), getGuangyaFileId: vi.fn(), getGuangyaFileName: vi.fn(), isGuangyaDir: vi.fn() }))
vi.mock('../../pikpak/dirfilelist', () => ({ apiPikPakFileList: vi.fn() }))
vi.mock('../../quark/dirfilelist', () => ({ apiQuarkFileList: vi.fn() }))

import { listMediaAcquisitionTargetLeafFiles, listMediaAcquisitionTransferBaselineFiles } from '../../services/mediaAcquisition/targetSnapshot'

const target = {
  title: '千与千寻',
  year: 2001,
  mediaType: 'movie',
  targetPlatform: '115',
  targetUserId: '115-user',
  targetDriveId: 'drive115',
  targetParentFileId: 'drive115_root'
} as MediaAcquisitionTarget

describe('media acquisition target snapshot', () => {
  beforeEach(() => {
    apiDrive115FileList.mockReset()
  })

  it('reads only the final movie folder instead of recursively scanning sibling folders', async () => {
    apiDrive115FileList.mockImplementation(async (_userId: string, parentId: string | number) => {
      if (String(parentId) === '0') {
        return [
          { fid: 'movie-folder', pid: '0', fc: '0', fn: '千与千寻 (2001)' },
          { fid: 'unrelated-folder', pid: '0', fc: '0', fn: '其它电影' }
        ]
      }
      if (String(parentId) === 'movie-folder') return [{ fid: 'movie-file', pid: 'movie-folder', fc: '1', fn: 'Spirited.Away.2001.mkv', fs: 1024 }]
      throw new Error(`不应扫描无关目录：${parentId}`)
    })

    await expect(listMediaAcquisitionTargetLeafFiles(target)).resolves.toEqual([
      { id: 'movie-file', name: 'Spirited.Away.2001.mkv', path: '/千与千寻 (2001)/Spirited.Away.2001.mkv', size: 1024, parentId: 'movie-folder' }
    ])
    expect(apiDrive115FileList.mock.calls.map(call => String(call[1]))).toEqual(['0', 'movie-folder'])
  })

  it('returns an empty snapshot when the final movie folder does not exist', async () => {
    apiDrive115FileList.mockResolvedValue([{ fid: 'unrelated-folder', pid: '0', fc: '0', fn: '其它电影' }])

    await expect(listMediaAcquisitionTargetLeafFiles(target)).resolves.toEqual([])
    expect(apiDrive115FileList).toHaveBeenCalledTimes(1)
  })

  it('uses the final movie folder for the transfer baseline', async () => {
    apiDrive115FileList.mockImplementation(async (_userId: string, parentId: string | number) => {
      if (String(parentId) === '0') return [{ fid: 'movie-folder', pid: '0', fc: '0', fn: '千与千寻 (2001)' }, { fid: 'unrelated-folder', pid: '0', fc: '0', fn: '其它电影' }]
      if (String(parentId) === 'movie-folder') return [{ fid: 'movie-file', pid: 'movie-folder', fc: '1', fn: 'Spirited.Away.2001.mkv', fs: 1024 }]
      throw new Error(`转存基线不应扫描无关目录：${parentId}`)
    })

    await expect(listMediaAcquisitionTransferBaselineFiles(target)).resolves.toHaveLength(1)
    expect(apiDrive115FileList.mock.calls.map(call => String(call[1]))).toEqual(['0', 'movie-folder'])
  })
})
