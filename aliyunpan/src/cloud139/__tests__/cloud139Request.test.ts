import { afterEach, describe, expect, it, vi } from 'vitest'

vi.mock('../../user/userdal', () => ({
  default: {
    GetUserToken: vi.fn(() => ({
      access_token: 'token-value',
      user_id: 'cloud139_13800138000',
      user_name: '13800138000',
      nick_name: '139云盘 13800138000'
    })),
    GetUserTokenFromDB: vi.fn()
  }
}))

vi.mock('../../utils/message', () => ({
  default: {
    error: vi.fn()
  }
}))

describe('cloud139Request', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('uses the routed personal cloud host for file list requests', async () => {
    const fetchMock = vi.fn(async (url: string) => {
      if (url === 'https://user-njs.yun.139.com/user/route/qryRoutePolicy') {
        return {
          ok: true,
          json: async () => ({
            success: true,
            data: {
              routePolicyList: [
                {
                  modName: 'personal',
                  httpsUrl: 'https://ose.caiyun.feixin.10086.cn/orchestration'
                }
              ]
            }
          })
        }
      }
      return {
        ok: true,
        json: async () => ({
          success: true,
          data: {
            items: [
              {
                fileId: 'folder-id',
                name: '电影',
                type: 'folder',
                updatedAt: '2026-07-13T10:00:00.000+08:00',
                createdAt: '2026-07-13T10:00:00.000+08:00'
              }
            ]
          }
        })
      }
    })
    vi.stubGlobal('fetch', fetchMock)

    const { apiCloud139FileList } = await import('../dirfilelist')
    const list = await apiCloud139FileList('cloud139_15558182007', 'cloud139_root')

    expect(list).toHaveLength(1)
    expect(fetchMock).toHaveBeenCalledTimes(2)
    const [routeUrl] = fetchMock.mock.calls[0] as unknown as [string, { headers: Record<string, string> }]
    const [listUrl, init] = fetchMock.mock.calls[1] as unknown as [string, { headers: Record<string, string> }]
    expect(routeUrl).toBe('https://user-njs.yun.139.com/user/route/qryRoutePolicy')
    expect(listUrl).toBe('https://ose.caiyun.feixin.10086.cn/orchestration/file/list')
    expect(listUrl).not.toContain('/orchestration/orchestration/')
    expect(init.headers['mcloud-sign']).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2},[A-Za-z0-9]{16},[A-F0-9]{32}$/)
    expect(init.headers.Authorization).toBe('Basic token-value')
  })

  it('builds personal urls without duplicating orchestration', async () => {
    const { cloud139PersonalUrl } = await import('../dirfilelist')

    expect(cloud139PersonalUrl('https://example.com/orchestration', '/file/list')).toBe('https://example.com/orchestration/file/list')
    expect(cloud139PersonalUrl('https://example.com', '/file/list')).toBe('https://example.com/orchestration/file/list')
  })

  it('uses the hcy file list path returned by route policy', async () => {
    const fetchMock = vi.fn(async (url: string) => {
      if (url === 'https://user-njs.yun.139.com/user/route/qryRoutePolicy') {
        return {
          ok: true,
          json: async () => ({
            success: true,
            data: {
              routePolicyList: [
                {
                  modName: 'personal',
                  httpsUrl: 'https://personal-kd-njs.yun.139.com/hcy'
                }
              ]
            }
          })
        }
      }
      if (url === 'https://personal-kd-njs.yun.139.com/hcy/file/list') {
        return {
          ok: true,
          json: async () => ({
            success: true,
            data: {
              items: [
                {
                  fileId: 'new-folder-id',
                  name: '电影',
                  type: 'folder',
                  updatedAt: '2026-07-13T10:00:00.000+08:00',
                  createdAt: '2026-07-13T10:00:00.000+08:00'
                }
              ]
            }
          })
        }
      }
      return {
        ok: false,
        status: 500,
        json: async () => undefined
      }
    })
    vi.stubGlobal('fetch', fetchMock)

    const { apiCloud139FileList } = await import('../dirfilelist')
    const list = await apiCloud139FileList('cloud139_13800138000', 'cloud139_root')

    expect(list).toHaveLength(1)
    expect(fetchMock).toHaveBeenCalledTimes(2)
    expect(fetchMock.mock.calls[1][0]).toBe('https://personal-kd-njs.yun.139.com/hcy/file/list')
  })

  it('tries fallback candidates when the routed file list host returns route not found', async () => {
    const fetchMock = vi.fn(async (url: string) => {
      if (url === 'https://user-njs.yun.139.com/user/route/qryRoutePolicy') {
        return {
          ok: true,
          json: async () => ({
            success: true,
            data: {
              routePolicyList: [
                {
                  modName: 'personal',
                  httpsUrl: 'https://personal-kd-njs.yun.139.com/hcy'
                }
              ]
            }
          })
        }
      }
      if (url === 'https://personal-kd-njs.yun.139.com/hcy/file/list') {
        return {
          ok: false,
          status: 404,
          json: async () => ({ error_msg: '404 Route Not Found' })
        }
      }
      if (url === 'https://personal-kd-njs.yun.139.com/orchestration/file/list') {
        return {
          ok: true,
          json: async () => ({
            success: true,
            data: {
              items: [
                {
                  fileId: 'new-file-id',
                  name: 'movie.mp4',
                  type: 'file',
                  size: 1024,
                  updatedAt: '2026-07-13T10:00:00.000+08:00',
                  createdAt: '2026-07-13T10:00:00.000+08:00'
                }
              ]
            }
          })
        }
      }
      return {
        ok: true,
        json: async () => ({
          success: true,
          data: {
            getDiskResult: {
              catalogList: [],
              contentList: [
                {
                  contentID: 'old-file-id',
                  contentName: 'movie.mp4',
                  contentSize: 1024,
                  updateTime: '20260713225311',
                  createTime: '20260713225311'
                }
              ]
            }
          }
        })
      }
    })
    vi.stubGlobal('fetch', fetchMock)

    const { apiCloud139FileList } = await import('../dirfilelist')
    const list = await apiCloud139FileList('cloud139_15558182008', 'cloud139_root')

    expect(list).toHaveLength(1)
    expect(fetchMock).toHaveBeenCalledTimes(3)
    expect(fetchMock.mock.calls[2][0]).toBe('https://personal-kd-njs.yun.139.com/orchestration/file/list')
  })

  it('builds personal url candidates for the hcy file list path', async () => {
    const { cloud139PersonalUrls } = await import('../dirfilelist')

    expect(cloud139PersonalUrls('https://personal-kd-njs.yun.139.com/hcy', '/file/list')).toEqual([
      'https://personal-kd-njs.yun.139.com/hcy/file/list',
      'https://personal-kd-njs.yun.139.com/orchestration/file/list'
    ])
  })
})
