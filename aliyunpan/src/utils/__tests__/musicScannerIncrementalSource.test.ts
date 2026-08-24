import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

vi.stubGlobal('self', globalThis)

const musicStore = vi.hoisted(() => ({
  sources: [] as Array<{ id: string }>,
  appendTracks: vi.fn(),
  markScanFinished: vi.fn(),
  reconcileSource: vi.fn(),
  saveSource: vi.fn(),
  setIsScanning: vi.fn(),
  setScanProgress: vi.fn()
}))

const oldAudio = {
  category: 'audio',
  drive_id: 'aliyun-drive',
  file_id: 'old-audio',
  isDir: false,
  name: 'Old Song.mp3',
  parent_file_id: 'root',
  size: 1,
  time: 1
}

vi.mock('../../store/musiclibrary', () => ({ default: () => musicStore }))
vi.mock('../../aliapi/filewalk', () => ({
  default: {
    ApiWalkFilePages: vi.fn(() => (async function* () { yield [oldAudio] })())
  }
}))
vi.mock('../../setting/settingstore', () => ({ default: () => ({ uiMusicAutoScanFolders: [], updateStore: vi.fn() }) }))

let MusicScanner: typeof import('../musicScanner').default

beforeAll(async () => {
  MusicScanner = (await import('../musicScanner')).default
})

describe('MusicScanner incremental sources', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    musicStore.sources = []
  })

  it('fully scans a newly discovered Aliyun drive instead of applying another source global timestamp', async () => {
    await MusicScanner.getInstance().scanUser({
      access_token: 'token',
      default_drive_id: 'aliyun-drive',
      tokenfrom: 'aliyun',
      user_id: 'aliyun-user'
    } as any, false, Date.now())

    expect(musicStore.appendTracks).toHaveBeenCalledWith([expect.objectContaining({ file_id: 'old-audio' })])
    expect(musicStore.reconcileSource).toHaveBeenCalledOnce()
    expect(musicStore.saveSource).toHaveBeenCalledOnce()
  })
})
