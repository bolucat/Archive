import { describe, expect, it, vi } from 'vitest'
import { BOX_DEFAULT_REDIRECT_URL, BOX_SCOPE, buildBoxAuthUrl, refreshBoxAccessToken } from '../auth'

describe('Box auth helpers', () => {
  it('builds Box authorize URL with PKCE and app protocol redirect', async () => {
    const url = await buildBoxAuthUrl('client-id', 'verifier', 'state-1')
    const parsed = new URL(url)

    expect(parsed.origin + parsed.pathname).toBe('https://account.box.com/api/oauth2/authorize')
    expect(parsed.searchParams.get('client_id')).toBe('client-id')
    expect(parsed.searchParams.get('response_type')).toBe('code')
    expect(parsed.searchParams.get('redirect_uri')).toBe(BOX_DEFAULT_REDIRECT_URL)
    expect(parsed.searchParams.get('code_challenge_method')).toBe('S256')
    expect(parsed.searchParams.get('code_challenge')).toBeTruthy()
    expect(parsed.searchParams.get('state')).toBe('state-1')
    expect(parsed.searchParams.get('scope')).toBe(BOX_SCOPE)
  })

  it('deduplicates concurrent refreshes because Box refresh tokens are single-use', async () => {
    const fetchMock = vi.fn().mockImplementation(async (url: string) => {
      if (url.includes('/oauth2/token')) {
        await new Promise((resolve) => setTimeout(resolve, 5))
        return { ok: true, json: async () => ({ access_token: 'access-2', refresh_token: 'refresh-2', expires_in: 3600 }) }
      }
      return { ok: true, json: async () => ({ id: 'box-account', name: 'Box' }) }
    })
    vi.stubGlobal('fetch', fetchMock)

    const token = { tokenfrom: 'box', user_id: 'box-account', device_id: 'client-id', refresh_token: 'refresh-1', access_token: 'access-1', expires_in: 3600, token_type: 'Bearer' } as any
    const [first, second] = await Promise.all([refreshBoxAccessToken(token), refreshBoxAccessToken(token)])

    expect(first?.refresh_token).toBe('refresh-2')
    expect(second).toBe(first)
    expect(fetchMock.mock.calls.filter((call: any[]) => String(call[0]).includes('/oauth2/token'))).toHaveLength(1)
  })
})
