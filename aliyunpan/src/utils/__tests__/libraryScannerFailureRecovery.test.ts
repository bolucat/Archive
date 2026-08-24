import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

vi.stubGlobal('self', globalThis)

const bookStore = vi.hoisted(() => ({
  appendBooks: vi.fn(),
  markScanFinished: vi.fn(),
  reconcileSource: vi.fn(),
  saveSource: vi.fn(),
  setIsScanning: vi.fn(),
  setScanProgress: vi.fn()
}))

const musicStore = vi.hoisted(() => ({
  appendTracks: vi.fn(),
  markScanFinished: vi.fn(),
  reconcileSource: vi.fn(),
  saveSource: vi.fn(),
  setIsScanning: vi.fn(),
  setScanProgress: vi.fn()
}))

const users = vi.hoisted(() => [{
  access_token: 'token',
  default_drive_id: 'quark',
  tokenfrom: 'quark',
  user_id: 'quark_test'
}])

vi.mock('../../store/booklibrary', () => ({
  default: () => bookStore,
  parseBookMeta: (name: string) => ({ title: name, author: '未知作者', metadata_source: 'filename' })
}))
vi.mock('../../store/musiclibrary', () => ({ default: () => musicStore }))
vi.mock('../../setting/settingstore', () => ({ default: () => ({ uiMusicAutoScanFolders: [], updateStore: vi.fn() }) }))
vi.mock('../../user/userdal', () => ({ default: { GetUserListFromDB: vi.fn(async () => users), GetUserToken: vi.fn(() => users[0]) } }))
vi.mock('../providerFolderList', () => ({
  isThirdPartyProviderFolder: () => true,
  iterateProviderFolderPages: async function* () {
    throw new Error('provider page failed')
  },
  listProviderFolderItems: vi.fn()
}))

let BookScanner: typeof import('../bookScanner').default
let MusicScanner: typeof import('../musicScanner').default

beforeAll(async () => {
  BookScanner = (await import('../bookScanner')).default
  MusicScanner = (await import('../musicScanner')).default
})

describe('library scanner failure recovery', () => {
  beforeEach(() => vi.clearAllMocks())

  it('does not reconcile a book source after provider pagination fails', async () => {
    await expect(BookScanner.getInstance().scanUser(users[0] as any)).rejects.toThrow('provider page failed')

    expect(bookStore.reconcileSource).not.toHaveBeenCalled()
    expect(bookStore.saveSource).not.toHaveBeenCalled()
    expect(bookStore.markScanFinished).not.toHaveBeenCalled()
  })

  it('does not reconcile a music source after provider pagination fails', async () => {
    await expect(MusicScanner.getInstance().scanUser(users[0] as any)).rejects.toThrow('provider page failed')

    expect(musicStore.reconcileSource).not.toHaveBeenCalled()
    expect(musicStore.saveSource).not.toHaveBeenCalled()
    expect(musicStore.markScanFinished).not.toHaveBeenCalled()
  })

  it('continues scanning the remaining Aliyun drives when one drive fails', async () => {
    const scanner = BookScanner.getInstance() as any
    const collect = vi.spyOn(scanner, 'bfsCollect')
      .mockRejectedValueOnce(new Error('default drive failed'))
      .mockResolvedValueOnce(undefined)
    const token = {
      access_token: 'token',
      default_drive_id: 'default-drive',
      resource_drive_id: 'resource-drive',
      tokenfrom: 'aliyun',
      user_id: 'aliyun_test'
    }

    await expect(scanner.scanUser(token)).resolves.toBeUndefined()

    expect(collect).toHaveBeenCalledTimes(2)
    expect(bookStore.reconcileSource).toHaveBeenCalledTimes(1)
    expect(bookStore.saveSource).toHaveBeenCalledTimes(1)
    collect.mockRestore()
  })

  it.each([
    ['book', () => BookScanner.getInstance()],
    ['music', () => MusicScanner.getInstance()]
  ])('does not truncate %s libraries at nine nested folders', async (_kind, getScanner) => {
    const scanner = getScanner() as any
    const pages = vi.spyOn(scanner, 'iterateFolderPages').mockImplementation(async function* () {
      yield []
    })
    const folder = { drive_id: 'drive', file_id: 'folder', isDir: true, name: 'deep folder' }

    await expect(scanner.bfsCollect(folder, 'user', 'drive', '', 'deep', { scanned: 0, found: 0, seen: new Set<string>() }, 9, 'source')).resolves.toBeUndefined()

    pages.mockRestore()
  })
})
