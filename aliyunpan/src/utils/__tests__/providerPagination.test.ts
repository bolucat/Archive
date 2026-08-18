import { describe, expect, it, vi } from 'vitest'

const adapters = vi.hoisted(() => {
  const result = () => Promise.resolve({ items: [], total: 0 })
  return {
    baidu: vi.fn(result), cloud123: vi.fn(result), cloud139: vi.fn(result), cloud189: vi.fn(result), drive115: vi.fn(result), dropbox: vi.fn(result), google: vi.fn(result), guangya: vi.fn(result), onedrive: vi.fn(result), pikpak: vi.fn(result), quark: vi.fn(result), box: vi.fn(result)
  }
})

vi.mock('../../cloudbaidu/adapter', () => ({ listBaiduItems: adapters.baidu }))
vi.mock('../../cloud123/adapter', () => ({ listCloud123Items: adapters.cloud123 }))
vi.mock('../../cloud139/adapter', () => ({ listCloud139Items: adapters.cloud139 }))
vi.mock('../../cloud189/adapter', () => ({ listCloud189Items: adapters.cloud189 }))
vi.mock('../../cloud115/adapter', () => ({ listDrive115Items: adapters.drive115 }))
vi.mock('../../dropbox/adapter', () => ({ listDropboxItems: adapters.dropbox }))
vi.mock('../../google/adapter', () => ({ listGoogleItems: adapters.google }))
vi.mock('../../guangya/adapter', () => ({ listGuangyaItems: adapters.guangya }))
vi.mock('../../onedrive/adapter', () => ({ listOneDriveItems: adapters.onedrive }))
vi.mock('../../pikpak/adapter', () => ({ listPikPakItems: adapters.pikpak }))
vi.mock('../../quark/adapter', () => ({ listQuarkItems: adapters.quark }))
vi.mock('../../box/adapter', () => ({ listBoxItems: adapters.box }))

import { listProviderItems } from '../../drive/providerList'
import { iterateProviderPages } from '../../drive/providerPagination'

describe('网盘主列表分页注册表', () => {
  it.each([
    ['baidu', '400', 'baidu', 400],
    ['cloud123', 'last-file-id', 'cloud123', 'last-file-id'],
    ['139', 'next-cursor', 'cloud139', 'next-cursor'],
    ['189', '3', 'cloud189', 3],
    ['115', '200', 'drive115', 200],
    ['pikpak', 'next-page-token', 'pikpak', 'next-page-token'],
    ['quark', '2', 'quark', 2],
    ['guangya', '4', 'guangya', 4],
    ['box', '500', 'box', 500],
    ['dropbox', 'dropbox-cursor', 'dropbox', 'dropbox-cursor'],
    ['onedrive', 'https://graph.microsoft.com/next', 'onedrive', 'https://graph.microsoft.com/next'],
    ['google', 'google-page-token', 'google', 'google-page-token']
  ] as const)('%s forwards its continuation cursor unchanged to its adapter', async (provider, cursor, adapter, expectedCursor) => {
    await listProviderItems(provider, 'user', 'drive', 'folder', true, cursor)

    expect(adapters[adapter]).toHaveBeenLastCalledWith('user', 'drive', 'folder', true, expectedCursor)
  })

  it('keeps the scroll-to-load wiring on every PanRight list view', async () => {
    const source = await import('../../pan/PanRight.vue?raw')
    const panDal = await import('../../pan/pandal.ts?raw')

    expect((source.default.match(/@scroll='handleListScroll'/g) || [])).toHaveLength(3)
    expect(source.default).toContain('distance < 120')
    expect(source.default).toContain('PanDAL.LoadMoreCurrentProviderItems()')
    expect(panDal.default).toContain('providerLoadingMore')
    expect(panDal.default).toContain('store.ListDataRaw = store.ListDataRaw.concat(items)')
  })
})

describe('统一分页状态机', () => {
  it('跨页去重，并在空 cursor 时结束', async () => {
    const requested: string[] = []
    const pages = [
      { items: [{ file_id: 'a' }, { file_id: 'b' }], total: 3, nextCursor: 'next' },
      { items: [{ file_id: 'b' }, { file_id: 'c' }], total: 3, nextCursor: '' }
    ]
    const received: string[][] = []

    for await (const page of iterateProviderPages(async cursor => {
      requested.push(cursor)
      return pages.shift() as any
    })) received.push(page.map(item => item.file_id))

    expect(requested).toEqual(['', 'next'])
    expect(received).toEqual([['a', 'b'], ['c']])
  })

  it('遇到重复 cursor 时停止，避免无限请求', async () => {
    const requested: string[] = []
    const pages = [
      { items: [{ file_id: 'a' }], total: 2, nextCursor: 'again' },
      { items: [{ file_id: 'a' }, { file_id: 'b' }], total: 2, nextCursor: 'again' }
    ]
    const received: string[][] = []

    for await (const page of iterateProviderPages(async cursor => {
      requested.push(cursor)
      return pages.shift() as any
    })) received.push(page.map(item => item.file_id))

    expect(requested).toEqual(['', 'again'])
    expect(received).toEqual([['a'], ['b']])
  })

  it('扫描器复用主列表注册表和分页状态机', async () => {
    const source = await import('../providerFolderList.ts?raw')

    expect(source.default).toContain("import { listProviderItems } from '../drive/providerList'")
    expect(source.default).toContain("import { iterateProviderPages } from '../drive/providerPagination'")
    expect(source.default).toContain('skipThumbnailHydration: true')
  })
})
