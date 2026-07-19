import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  apiQuarkShareCreate,
  apiQuarkShareDetail,
  apiQuarkShareFileList,
  apiQuarkShareList,
  apiQuarkSaveShareFilesBatch,
  apiQuarkShareToken,
  parseQuarkShareLink
} from '../share'

vi.mock('../../user/userdal', () => ({
  default: {
    GetUserToken: () => ({ access_token: '__uid=u1; __kps=kps1', user_id: 'quark_u1' }),
    GetUserTokenFromDB: async () => null,
    GetUserListFromDB: async () => [{ access_token: '__uid=u1; __kps=kps1', user_id: 'quark_u1', tokenfrom: 'quark' }],
    SaveUserToken: vi.fn()
  }
}))

vi.mock('../../utils/message', () => ({
  default: {
    error: vi.fn(),
    info: vi.fn()
  }
}))

afterEach(() => vi.unstubAllGlobals())

describe('parseQuarkShareLink', () => {
  it('parses Quark share urls and passwords', () => {
    expect(parseQuarkShareLink('https://pan.quark.cn/s/abcd1234?pwd=9xyz')).toEqual({
      id: 'quark:abcd1234',
      pwd: '9xyz'
    })
    expect(parseQuarkShareLink('夸克链接 https://pan.quark.cn/s/abcd1234 提取码：123456')).toEqual({
      id: 'quark:abcd1234',
      pwd: '123456'
    })
    expect(parseQuarkShareLink('quark://share/abcd1234')).toEqual({
      id: 'quark:abcd1234',
      pwd: ''
    })
  })
})

describe('apiQuarkShareCreate', () => {
  it('creates a share with QuarkPan request sequence', async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ status: 200, data: { task_id: 'task1' } })
      })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ status: 200, data: { status: 2, share_id: 'share1' } })
      })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ status: 200, data: { share_url: 'https://pan.quark.cn/s/share1', title: 'Movie', passcode: '9xyz' } })
      })
    vi.stubGlobal('fetch', fetchMock)

    const result = await apiQuarkShareCreate('quark_u1', '2030-01-01T00:00:00.000Z', '9xyz', 'Movie', ['fid1'])

    expect(typeof result).toBe('object')
    expect((result as any).share_id).toBe('quark:share1')
    expect((result as any).share_pwd).toBe('9xyz')
    expect(String(fetchMock.mock.calls[0][0])).toContain('/share?')
    expect(JSON.parse(fetchMock.mock.calls[0][1].body as string)).toMatchObject({
      fid_list: ['fid1'],
      title: 'Movie',
      url_type: 2,
      expired_type: 2,
      passcode: '9xyz'
    })
    expect(String(fetchMock.mock.calls[1][0])).toContain('/task?')
    expect(String(fetchMock.mock.calls[2][0])).toContain('/share/password?')
  })
})

describe('apiQuarkShareList', () => {
  it('uses Quark my share endpoint', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        status: 200,
        data: {
          list: [{
            share_id: 'share1',
            share_url: 'https://pan.quark.cn/s/share1',
            title: 'Movie',
            created_at: 1710000000000,
            expired_at: 0
          }]
        }
      })
    })
    vi.stubGlobal('fetch', fetchMock)

    const list = await apiQuarkShareList('quark_u1')

    expect(list[0].share_id).toBe('quark:share1')
    expect(String(fetchMock.mock.calls[0][0])).toContain('/share/mypage/detail?')
    expect(String(fetchMock.mock.calls[0][0])).toContain('_order_field=created_at')
  })
})

describe('apiQuarkShareToken and detail', () => {
  it('uses Quark share base URL', async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ status: 200, data: { stoken: 'st1' } })
      })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ status: 200, data: { list: [] } })
      })
    vi.stubGlobal('fetch', fetchMock)

    const token = await apiQuarkShareToken('quark:share1', '9xyz', 'quark_u1')
    const detail = await apiQuarkShareDetail('quark:share1', token, '0', 1, 'quark_u1')

    expect(token).toBe('st1')
    expect(detail.error).toBe('')
    expect(String(fetchMock.mock.calls[0][0])).toContain('https://drive.quark.cn/1/clouddrive/share/sharepage/token?')
    expect(JSON.parse(fetchMock.mock.calls[0][1].body as string)).toMatchObject({
      pwd_id: 'share1',
      passcode: '9xyz',
      support_visit_limit_private_share: true
    })
    expect(fetchMock.mock.calls[0][1].headers).toMatchObject({ cookie: '__uid=u1; __kps=kps1' })
    expect(String(fetchMock.mock.calls[1][0])).toContain('https://drive.quark.cn/1/clouddrive/share/sharepage/detail?')
    expect(fetchMock.mock.calls[1][1].headers).toMatchObject({ cookie: '__uid=u1; __kps=kps1' })
  })

  it('loads all share pages and keeps pagination separate from errors', async () => {
    const firstPage = Array.from({ length: 100 }, (_, index) => ({ fid: `f${index}`, file_name: `f${index}.mkv`, file_type: 1, share_fid_token: `t${index}` }))
    const fetchMock = vi.fn()
      .mockResolvedValueOnce({ ok: true, json: async () => ({ status: 200, data: { list: firstPage, metadata: { _total: 101 } } }) })
      .mockResolvedValueOnce({ ok: true, json: async () => ({ status: 200, data: { list: [{ fid: 'f100', file_name: 'f100.mkv', file_type: 1, share_fid_token: 't100' }], metadata: { _total: 101 } } }) })
    vi.stubGlobal('fetch', fetchMock)

    const result = await apiQuarkShareFileList('quark:share1', 'st1', 'root')

    expect(result.error).toBe('')
    expect(result.items).toHaveLength(101)
    expect(String(fetchMock.mock.calls[0][0])).toContain('pwd_id=share1')
    expect(String(fetchMock.mock.calls[1][0])).toContain('_page=2')
  })
})

describe('apiQuarkSaveShareFilesBatch', () => {
  it('returns the synchronous task failure instead of reporting a false success', async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce({ ok: true, json: async () => ({ status: 200, data: { list: [{ fid: 'f1', file_name: 'movie.mkv', file_type: 1, share_fid_token: 'token1' }] } }) })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          status: 200,
          code: 0,
          data: {
            task_id: 'task1',
            task_sync: true,
            task_resp: { status: 400, code: 32003, message: 'capacity limit[{0}]', metadata: { missing_capacity: 165011263325 } }
          }
        })
      })
    vi.stubGlobal('fetch', fetchMock)

    const listed = await apiQuarkShareFileList('quark:share1', 'st1', 'root', 'quark_u1')
    expect(listed.error).toBe('')
    const result = await apiQuarkSaveShareFilesBatch('quark:share1', 'st1', 'quark_u1', '0', ['f1'])

    expect(result).toContain('夸克网盘容量不足')
  })
})
