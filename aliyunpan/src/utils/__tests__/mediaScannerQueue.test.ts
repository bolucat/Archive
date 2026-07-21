import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

const mediaStore = {
  mediaItems: [] as any[],
  addFolder: vi.fn(),
  pruneOrphanDuplicateFolders: vi.fn(),
  setScanning: vi.fn(),
  setScanProgress: vi.fn()
}

const storage = new Map<string, string>()

vi.mock('../../store/medialibrary', () => ({ useMediaLibraryStore: () => mediaStore }))
vi.mock('../../store', () => ({ usePanTreeStore: () => ({ drive_id: 'quark', user_id: 'quark_user' }) }))
vi.mock('../../user/userdal', () => ({
  default: {
    GetUserToken: vi.fn().mockReturnValue({ user_id: 'quark_user', tokenfrom: 'quark' }),
    GetUserListFromDB: vi.fn().mockResolvedValue([])
  }
}))
vi.mock('../tmdb', () => ({
  TmdbService: { getInstance: () => ({}) },
  tmdbImageUrl: vi.fn()
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

  it('rejects a silent Agent scan when provider traversal fails', async () => {
    const scanner = new MediaScanner()
    ;(scanner as any).getFolderItemsWithRetry = vi.fn().mockRejectedValue(new Error('provider list failed'))

    await expect(scanner.scanFolder(folder('agent-import-folder'), 'quark', { silent: true })).rejects.toThrow('provider list failed')
    expect(mediaStore.setScanning).toHaveBeenLastCalledWith(false)
  })
})
