import { afterEach, describe, expect, it, vi } from 'vitest'
import { fetchCloud123JsonWithAuthRetry } from '../../cloud123/request'

afterEach(() => vi.unstubAllGlobals())
describe('123 directory token rejection recovery', () => {
  it.each([401, 200])('refreshes a rejected token once (HTTP %s)', async (status) => {
    const fetcher = vi.fn().mockResolvedValueOnce(new Response(JSON.stringify({ code: 401, message: 'access Token Invalid' }), { status }))
      .mockResolvedValueOnce(new Response(JSON.stringify({ code: 0, data: { fileList: [{ fileId: 1 }] } })))
    vi.stubGlobal('fetch', fetcher)
    const refresh = vi.fn(async () => ({ access_token: 'new-token' }))
    const result = await fetchCloud123JsonWithAuthRetry('https://example.test/list?parentFileId=0', { access_token: 'old-token' }, refresh)
    expect(result.data.code).toBe(0)
    expect(refresh).toHaveBeenCalledOnce()
    expect(fetcher).toHaveBeenLastCalledWith('https://example.test/list?parentFileId=0', { headers: { 'Content-Type': 'application/json', Authorization: 'Bearer new-token', Platform: 'open_platform' } })
  })
  it('does not refresh on permission errors or successful empty directories', async () => {
    const fetcher = vi.fn().mockResolvedValueOnce(new Response(JSON.stringify({ code: 403, message: 'permission denied' })))
      .mockResolvedValueOnce(new Response(JSON.stringify({ code: 0, data: { fileList: [] } })))
    vi.stubGlobal('fetch', fetcher)
    const refresh = vi.fn()
    await fetchCloud123JsonWithAuthRetry('https://example.test/list', { access_token: 'token' }, refresh)
    await fetchCloud123JsonWithAuthRetry('https://example.test/list', { access_token: 'token' }, refresh)
    expect(refresh).not.toHaveBeenCalled()
  })
  it('preserves rejection when refreshing fails or the new token is also invalid', async () => {
    const fetcher = vi.fn(async () => new Response(JSON.stringify({ code: 401, message: 'access Token Invalid' })))
    vi.stubGlobal('fetch', fetcher)
    const failed = vi.fn(async () => null)
    expect((await fetchCloud123JsonWithAuthRetry('https://example.test/list', { access_token: 'old' }, failed)).data.code).toBe(401)
    expect(fetcher).toHaveBeenCalledOnce()
    fetcher.mockClear()
    const refresh = vi.fn(async () => ({ access_token: 'still-invalid' }))
    expect((await fetchCloud123JsonWithAuthRetry('https://example.test/list', { access_token: 'old' }, refresh)).data.code).toBe(401)
    expect(fetcher).toHaveBeenCalledTimes(2)
    expect(refresh).toHaveBeenCalledOnce()
  })
})
