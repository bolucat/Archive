import { describe, expect, it } from 'vitest'
import { ensureMpvLoopbackProxyBypass } from '../mpvProxyEnvironment'

describe('MPV loopback proxy bypass', () => {
  it('bypasses internal servers without changing the external proxy', () => {
    const env: Record<string, string | undefined> = { http_proxy: 'http://127.0.0.1:7892', https_proxy: 'http://127.0.0.1:7892' }
    ensureMpvLoopbackProxyBypass(env)
    expect(env.no_proxy).toBe('localhost,127.0.0.1,::1')
    expect(env.NO_PROXY).toBe(env.no_proxy)
    expect(env.http_proxy).toBe('http://127.0.0.1:7892')
    expect(env.https_proxy).toBe('http://127.0.0.1:7892')
  })

  it('preserves both spellings of existing bypass rules and is idempotent', () => {
    const env = { no_proxy: ' .example.com,127.0.0.1 ', NO_PROXY: 'internal.example,::1' }
    ensureMpvLoopbackProxyBypass(env)
    expect(env.no_proxy).toBe('.example.com,127.0.0.1,internal.example,::1,localhost')
    ensureMpvLoopbackProxyBypass(env)
    expect(env.NO_PROXY).toBe('.example.com,127.0.0.1,internal.example,::1,localhost')
  })

  it('preserves an explicitly configured universal bypass', () => {
    const env = { NO_PROXY: '*' } as Record<string, string | undefined>
    ensureMpvLoopbackProxyBypass(env)
    expect(env.no_proxy?.split(',')).toContain('*')
  })
})
