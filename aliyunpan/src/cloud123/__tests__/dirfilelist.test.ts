import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('../../utils/message', () => ({ default: { error: vi.fn() } }))
vi.mock('../../config', () => ({ default: {} }))
vi.mock('../auth', () => ({ getCloud123Token: vi.fn().mockResolvedValue({ access_token: 'token' }) }))

import { apiCloud123DirectoryFileList } from '../dirfilelist'

describe('123 云盘目录列表', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('loads every cursor page instead of stopping after the first 100 files', async () => {
    const first = Array.from({ length: 100 }, (_, index) => ({ fileId: index + 1, filename: `${index + 1}.txt`, parentFileId: 0, type: 0, size: 1, category: 0, status: 0, trashed: 0 }))
    const second = Array.from({ length: 20 }, (_, index) => ({ fileId: index + 101, filename: `${index + 101}.txt`, parentFileId: 0, type: 0, size: 1, category: 0, status: 0, trashed: 0 }))
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce({ ok: true, json: async () => ({ code: 0, data: { lastFileId: 100, fileList: first } }) } as Response)
      .mockResolvedValueOnce({ ok: true, json: async () => ({ code: 0, data: { lastFileId: -1, fileList: second } }) } as Response)

    const items = await apiCloud123DirectoryFileList('user', '0')

    expect(items).toHaveLength(120)
    expect(fetchMock).toHaveBeenCalledTimes(2)
  })
})
