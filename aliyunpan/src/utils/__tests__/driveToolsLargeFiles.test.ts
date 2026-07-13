import { describe, expect, it, vi } from 'vitest'
import { scanDriveLargeFiles } from '../drive-tools/largeFiles'

vi.mock('../drive-tools/directLinks', () => ({
  listDriveToolChildren: vi.fn(async (_userId: string, _driveId: string, fileId: string) => {
    if (fileId === 'root') {
      return [
        { file_id: 'dir1', drive_id: 'drive', parent_file_id: 'root', name: 'Movies', isDir: true, size: 0 },
        { file_id: 'small', drive_id: 'drive', parent_file_id: 'root', name: 'small.txt', isDir: false, size: 10 * 1024 * 1024, ext: '.txt', category: 'doc', icon: 'iconfile_txt' }
      ]
    }
    if (fileId === 'dir1') {
      return [
        { file_id: 'big-video', drive_id: 'drive', parent_file_id: 'dir1', name: 'movie.mkv', isDir: false, size: 2 * 1024 * 1024 * 1024, ext: '.mkv', category: 'video', icon: 'iconfile_video' },
        { file_id: 'big-zip', drive_id: 'drive', parent_file_id: 'dir1', name: 'archive.zip', isDir: false, size: 2 * 1024 * 1024 * 1024, ext: 'zip', category: 'others', icon: 'iconfile_zip' }
      ]
    }
    return []
  })
}))

describe('scanDriveLargeFiles', () => {
  it('scans generic drive targets and filters by mode', async () => {
    const targets = [{ userId: 'u', driveId: 'drive', rootId: 'root', name: 'Drive' }]
    await expect(scanDriveLargeFiles(targets, 'video')).resolves.toMatchObject({
      scannedDirs: 2,
      scannedFiles: 3,
      files: [{ fileId: 'big-video', path: 'Drive/Movies/movie.mkv' }]
    })
    await expect(scanDriveLargeFiles(targets, 'zip')).resolves.toMatchObject({
      files: [{ fileId: 'big-zip' }]
    })
  })
})
