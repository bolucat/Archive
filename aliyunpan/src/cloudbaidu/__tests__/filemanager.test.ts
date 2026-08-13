import { beforeEach, describe, expect, it, vi } from 'vitest'

const getBaiduToken = vi.hoisted(() => vi.fn())
const request = vi.hoisted(() => vi.fn())

vi.mock('../auth', () => ({ getBaiduToken }))

import { apiBaiduDelete } from '../filemanager'
import { getProviderCapabilities } from '../../services/agent/providerCapabilities'

describe('apiBaiduDelete', () => {
  beforeEach(() => {
    getBaiduToken.mockResolvedValue({ access_token: 'token' })
    request.mockReset()
    vi.stubGlobal('fetch', request)
  })

  it('accepts the asynchronous task response returned by filemanager delete', async () => {
    request.mockResolvedValue({ ok: true, json: async () => ({ errno: 0, info: [], taskid: 109078799891710 }) })

    await expect(apiBaiduDelete('baidu_user', ['/测试目录/test/西瓜书.pdf'])).resolves.toEqual(['/测试目录/test/西瓜书.pdf'])

    const [url, options] = request.mock.calls[0]
    expect(url).toContain('method=filemanager')
    expect(url).toContain('opera=delete')
    expect(options.body.get('async')).toBe('2')
    expect(options.body.get('filelist')).toBe('["/测试目录/test/西瓜书.pdf"]')
  })

  it('advertises delete-to-trash without advertising unsupported recycle-bin actions', () => {
    const operations = getProviderCapabilities('baidu').operations
    expect(operations['trash.move']).toBe(true)
    expect(operations['trash.delete']).toBe(false)
    expect(operations['trash.restore']).toBe(false)
  })
})
