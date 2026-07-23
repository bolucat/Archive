import { afterEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  count: vi.fn(),
  listRoot: vi.fn(),
  post: vi.fn()
}))

vi.mock('../user', () => ({ default: { ApiUserDriveFileCount: mocks.count } }))
vi.mock('../trash', () => ({ default: { ApiDirFileListNoLock: mocks.listRoot } }))
vi.mock('../../utils/debuglog', () => ({ default: { mSaveWarning: vi.fn(), mSaveDanger: vi.fn() } }))
vi.mock('../../utils/utils', () => ({ MapValueToArray: (map: Map<unknown, unknown>) => Array.from(map.values()) }))
vi.mock('../alihttp', () => ({
  default: {
    IsSuccess: (code: number) => code >= 200 && code <= 300,
    HttpCodeBreak: (code: number) => (code >= 200 && code <= 300) || code === 400 || (code > 402 && code <= 428),
    Post: mocks.post
  }
}))

Object.assign(globalThis, {
  self: globalThis,
  window: globalThis,
  pinyinlite: (input: string) => input.split('').map((char) => [char])
})

describe('AliDirList.ApiFastAllDirListByPID', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('stops the full-directory scan when a batch request is rejected', async () => {
    const { default: AliDirList } = await import('../dirlist')
    mocks.count.mockResolvedValue(1)
    mocks.listRoot.mockResolvedValue({
      items: [{ file_id: 'folder-1', drive_id: 'drive-1', parent_file_id: 'root', name: 'Folder 1', time: 0, size: 0 }],
      itemsKey: new Set(),
      itemsTotal: 1,
      next_marker: '',
      m_user_id: 'user-1',
      m_drive_id: 'drive-1',
      dirID: 'resource_root',
      dirName: ''
    } as any)
    mocks.post
      .mockResolvedValueOnce({ code: 403, header: '', body: { message: 'Forbidden' } })
      .mockResolvedValueOnce({ code: 200, header: '', body: { items: [], next_marker: '' } })

    const result = await AliDirList.ApiFastAllDirListByPID('user-1', 'drive-1', 'resource_root')

    expect(mocks.post).toHaveBeenCalledTimes(1)
    expect(result.next_marker).toBe('403')
  })
})
