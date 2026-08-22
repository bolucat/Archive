import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

const mediaStore = {
  mediaItems: [] as any[],
  folders: [] as any[],
  addFolder: vi.fn(),
  pruneOrphanDuplicateFolders: vi.fn(),
  beginPersistenceBatch: vi.fn(),
  checkpointPersistenceBatch: vi.fn(),
  endPersistenceBatch: vi.fn(),
  reconcileFolderSource: vi.fn(),
  removeFolder: vi.fn(),
  setScanning: vi.fn(),
  setScanProgress: vi.fn()
}

const storage = new Map<string, string>()

vi.mock('../../store/medialibrary', () => ({ useMediaLibraryStore: () => mediaStore }))
vi.mock('../../store', () => ({ usePanTreeStore: () => ({ drive_id: 'quark', user_id: 'quark_user' }) }))
vi.mock('../../setting/settingstore', () => ({ default: () => ({ mediaLibrarySubtitleScope: 'same-folder' }) }))
vi.mock('../../user/userdal', () => ({
  default: {
    GetUserToken: vi.fn().mockReturnValue({ user_id: 'quark_user', tokenfrom: 'quark' }),
    GetUserListFromDB: vi.fn().mockResolvedValue([])
  }
}))
class MockTmdbTransientError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'TmdbTransientError'
  }
}

vi.mock('../tmdb', () => ({
  TmdbService: { getInstance: () => ({}) },
  TmdbTransientError: MockTmdbTransientError,
  tmdbImageUrl: vi.fn()
}))
vi.mock('../db', () => ({
  default: {
    getIndexedMediaFileIds: vi.fn().mockResolvedValue(new Set()),
    getMediaLibraryFolderFileIds: vi.fn().mockResolvedValue([]),
    reconcileMediaLibraryFolder: vi.fn().mockResolvedValue(undefined)
  }
}))

let MediaScanner: typeof import('../mediaScanner').MediaScanner

beforeAll(async () => {
  vi.stubGlobal('self', globalThis)
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => storage.set(key, value),
    removeItem: (key: string) => storage.delete(key)
  })
  MediaScanner = (await import('../mediaScanner')).MediaScanner
})

const folder = (id: string) => ({
  drive_id: 'quark',
  file_id: id,
  parent_file_id: 'quark_root',
  name: id,
  path: `/${id}`,
  isDir: true
}) as any

describe('MediaScanner scan queue', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    storage.clear()
    mediaStore.mediaItems = []
  })

  it('runs an Agent silent scan after an existing media-library scan finishes', async () => {
    let releaseFirst!: () => void
    const firstItems = new Promise<any[]>((resolve) => {
      releaseFirst = () => resolve([])
    })
    const scanner = new MediaScanner()
    const getFolderItems = vi.fn()
      .mockImplementationOnce(() => firstItems)
      .mockResolvedValueOnce([])
    ;(scanner as any).getFolderItemsWithRetry = getFolderItems

    const firstScan = scanner.scanFolder(folder('background-folder'), 'quark', { incremental: true, silent: true })
    await vi.waitFor(() => expect(getFolderItems).toHaveBeenCalledTimes(1))

    const agentScan = scanner.scanFolder(folder('agent-import-folder'), 'quark', { incremental: true, silent: true })
    releaseFirst()
    await Promise.all([firstScan, agentScan])

    expect(getFolderItems).toHaveBeenCalledTimes(2)
    expect(getFolderItems.mock.calls[1][0].file_id).toBe('agent-import-folder')
  })

  it('does not expose an in-flight Agent scan as a resumable manual scrape', async () => {
    let finishScan!: () => void
    const items = new Promise<any[]>((resolve) => {
      finishScan = () => resolve([])
    })
    const scanner = new MediaScanner()
    ;(scanner as any).getFolderItemsWithRetry = vi.fn().mockReturnValue(items)

    const scan = scanner.scanFolder(folder('agent-import-folder'), 'quark', { incremental: false, silent: true })
    await vi.waitFor(() => expect((scanner as any).getFolderItemsWithRetry).toHaveBeenCalled())

    expect(storage.has('media_scan_checkpoint')).toBe(false)
    finishScan()
    await scan
  })

  it('does not add a video source when the folder contains no videos', async () => {
    const scanner = new MediaScanner()
    ;(scanner as any).getFolderItemsWithRetry = vi.fn().mockResolvedValue([])

    await scanner.scanFolder(folder('book-only-folder'), 'quark', { silent: true })

    expect(mediaStore.addFolder).not.toHaveBeenCalled()
  })

  it('rejects a silent Agent scan when provider traversal fails', async () => {
    const scanner = new MediaScanner()
    ;(scanner as any).getFolderItemsWithRetry = vi.fn().mockRejectedValue(new Error('provider list failed'))

    await expect(scanner.scanFolder(folder('agent-import-folder'), 'quark', { silent: true })).rejects.toThrow('provider list failed')
    expect(mediaStore.setScanning).toHaveBeenLastCalledWith(false)
    expect(mediaStore.beginPersistenceBatch).toHaveBeenCalledTimes(1)
    expect(mediaStore.endPersistenceBatch).toHaveBeenCalledTimes(1)
  })

  it('uses drive file IDs when deciding which files an incremental scan can skip', () => {
    mediaStore.mediaItems = [{
      id: 'tmdb-movie-id',
      driveFiles: [{ id: 'movie-file-id' }],
      seasons: [{ episodes: [{ driveFiles: [{ id: 'episode-file-id' }] }] }]
    }]
    const scanner = new MediaScanner()

    expect(Array.from((scanner as any).getIndexedDriveFileIds()).sort()).toEqual([':::episode-file-id', ':::movie-file-id'])
  })

  it('scopes media-source and file keys by provider, account, and drive', () => {
    const scanner = new MediaScanner()
    const first = (scanner as any).getScopedFolderKey({ driveServerId: 'quark', userId: 'quark_a', driveId: 'quark' }, '0')
    const second = (scanner as any).getScopedFolderKey({ driveServerId: 'drive115', userId: 'drive115_b', driveId: 'drive115' }, '0')
    const firstFile = (scanner as any).getScopedDriveFileKey({ driveServerId: 'quark', userId: 'quark_a', driveId: 'quark', id: '1' })
    const secondFile = (scanner as any).getScopedDriveFileKey({ driveServerId: 'quark', userId: 'quark_b', driveId: 'quark', id: '1' })

    expect(first).not.toBe(second)
    expect(firstFile).not.toBe(secondFile)
  })

  it('checkpoints a scan after 100 processed items', () => {
    const scanner = new MediaScanner()
    ;(scanner as any).resetPersistenceCheckpoint()
    ;(scanner as any).checkpointScanPersistence(99)
    expect(mediaStore.checkpointPersistenceBatch).not.toHaveBeenCalled()

    ;(scanner as any).checkpointScanPersistence(1)
    expect(mediaStore.checkpointPersistenceBatch).toHaveBeenCalledTimes(1)
  })

  it('automatically retries transient TMDB failures before completing a scan batch', async () => {
    vi.useFakeTimers()
    try {
      const scanner = new MediaScanner()
      const file = { id: 'retry-file', name: 'Retry.Movie.2026.mkv', path: '/Retry.Movie.2026.mkv', driveId: 'quark', driveServerId: 'quark', fileSize: 1 } as any
      const processFile = vi.spyOn(scanner as any, 'processVideoFileWithoutAI')
        .mockRejectedValueOnce(new MockTmdbTransientError('429'))
        .mockResolvedValueOnce(null)

      const result = (scanner as any).processVideoBatchWithTransientRetry([file], 'folder', 'folder-id')
      await vi.advanceTimersByTimeAsync(3000)

      await expect(result).resolves.toEqual({ unmatched: [], completed: [file], unresolved: [] })
      expect(processFile).toHaveBeenCalledTimes(2)
    } finally {
      vi.useRealTimers()
    }
  })
})
