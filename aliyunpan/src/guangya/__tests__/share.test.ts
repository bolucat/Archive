import { afterEach, describe, expect, it, vi } from 'vitest'

vi.mock('../auth', () => ({
  GUANGYA_API_URL: 'https://api.guangya.test',
  GUANGYA_WEB_URL: 'https://www.guangya.test',
  GUANGYA_USER_AGENT: 'BoxPlayer-Test',
  generateGuangyaDid: () => 'did',
  generateGuangyaTraceparent: () => 'traceparent'
}))
vi.mock('../dirfilelist', () => ({
  getGuangyaFileId: (item: any) => String(item.id || ''),
  getGuangyaFileName: (item: any) => String(item.name || ''),
  guangyaApiParentId: (value: string) => value,
  guangyaRequest: vi.fn(),
  isGuangyaDir: (item: any) => item.type === 0
}))

import { apiGuangyaShareFileList, apiGuangyaShareToken, parseGuangyaShareLink } from '../share'

afterEach(() => vi.unstubAllGlobals())

describe('Guangya share helpers', () => {
  it('decodes the internal share id when requesting an access token', async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true, json: async () => ({ data: { accessToken: 'share-token' } }) })
    vi.stubGlobal('fetch', fetchMock)

    await expect(apiGuangyaShareToken(parseGuangyaShareLink('https://www.guangyapan.com/s/share-id').id, '')).resolves.toBe('share-token')

    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toMatchObject({ shareId: 'share-id' })
  })

  it('loads every page from a shared folder', async () => {
    const firstPage = Array.from({ length: 100 }, (_, index) => ({ id: `file-${index}`, name: `${index}.mkv`, type: 1, size: 1 }))
    const fetchMock = vi.fn()
      .mockResolvedValueOnce({ ok: true, json: async () => ({ data: { list: firstPage, total: 101 } }) })
      .mockResolvedValueOnce({ ok: true, json: async () => ({ data: { list: [{ id: 'file-100', name: '100.mkv', type: 1, size: 1 }], total: 101 } }) })
    vi.stubGlobal('fetch', fetchMock)

    const result = await apiGuangyaShareFileList('guangya:share-id', 'share-token', 'root')

    expect(result.items).toHaveLength(101)
    expect(JSON.parse(fetchMock.mock.calls[1][1].body)).toMatchObject({ page: 2 })
  })
})
