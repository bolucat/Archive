import { afterEach, describe, expect, it, vi } from 'vitest'
import { createPlexServerConfigs, getPlexClientId, verifyPlexServerConfig } from '../plexAuth'
import type { PlexResource } from '../../types/mediaServerPlex'

const stubLocalStorage = (initial?: Record<string, string>) => {
  const values = new Map(Object.entries(initial || {}))
  const storage = {
    getItem: vi.fn((key: string) => values.get(key) ?? null),
    setItem: vi.fn((key: string, value: string) => {
      values.set(key, value)
    }),
    removeItem: vi.fn((key: string) => {
      values.delete(key)
    }),
    clear: vi.fn(() => {
      values.clear()
    })
  }
  vi.stubGlobal('localStorage', storage)
  return storage
}

describe('plexAuth', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('reuses the stored Plex client identifier', () => {
    stubLocalStorage({ boxplayer_plex_client_id: 'boxplayer-existing-client' })

    expect(getPlexClientId()).toBe('boxplayer-existing-client')
  })

  it('creates Plex server configs with a stable non-empty device id', () => {
    stubLocalStorage({ boxplayer_plex_client_id: 'boxplayer-stable-client' })
    const resources: PlexResource[] = [{
      name: 'Living Room Plex',
      accessToken: 'plex-token',
      clientIdentifier: 'plex-server-1',
      connections: [
        {
          uri: 'https://plex.example.com:32400',
          address: 'plex.example.com',
          protocol: 'https',
          port: 32400,
          local: false,
          relay: false
        }
      ]
    }]

    const [config] = createPlexServerConfigs(resources)

    expect(config.type).toBe('plex')
    expect(config.deviceId).toBe('boxplayer-stable-client')
    expect(config.baseUrl).toBe('https://plex.example.com:32400')
    expect(config.backupAddresses?.Remote).toBe('https://plex.example.com:32400')
  })

  it('prefers a local Plex connection while preserving remote fallback lines', () => {
    stubLocalStorage({ boxplayer_plex_client_id: 'boxplayer-stable-client' })
    const resources: PlexResource[] = [{
      name: 'Studio Plex',
      accessToken: 'plex-token',
      clientIdentifier: 'plex-server-2',
      connections: [
        {
          uri: 'https://remote.plex.example.com:32400',
          address: 'remote.plex.example.com',
          protocol: 'https',
          port: 32400,
          local: false,
          relay: false
        },
        {
          uri: 'http://192.168.1.20:32400',
          address: '192.168.1.20',
          protocol: 'http',
          port: 32400,
          local: true,
          relay: false
        }
      ]
    }]

    const [config] = createPlexServerConfigs(resources)

    expect(config.baseUrl).toBe('http://192.168.1.20:32400')
    expect(config.backupAddresses?.Local).toBe('http://192.168.1.20:32400')
    expect(config.backupAddresses?.Remote).toBe('https://remote.plex.example.com:32400')
  })

  it('verifies Plex server identity before saving', async () => {
    stubLocalStorage({ boxplayer_plex_client_id: 'boxplayer-stable-client' })
    vi.stubGlobal('fetch', vi.fn(async () => ({
      ok: true,
      json: async () => ({ MediaContainer: { friendlyName: 'Verified Plex' } })
    })))

    await expect(verifyPlexServerConfig({ baseUrl: 'http://192.168.1.20:32400', accessToken: 'token' })).resolves.toBe('Verified Plex')
  })

  it('rejects unreachable Plex server connections', async () => {
    stubLocalStorage({ boxplayer_plex_client_id: 'boxplayer-stable-client' })
    vi.stubGlobal('fetch', vi.fn(async () => ({
      ok: false,
      json: async () => ({})
    })))

    await expect(verifyPlexServerConfig({ baseUrl: 'http://192.168.1.20:32400', accessToken: 'token' })).rejects.toThrow('Plex 服务器连接验证失败')
  })
})
