import { describe, expect, it } from 'vitest'
import { buildWebDavDownloadUrl, getWebDavDownloadUrl, type WebDavConnectionConfig } from '../webdavClient'

const connection = (overrides: Partial<WebDavConnectionConfig> = {}): WebDavConnectionConfig => ({
  id: 'nas',
  kind: 'webdav',
  name: 'NAS',
  url: 'https://example.com/dav',
  username: 'alice',
  password: 'secret',
  rootPath: '/',
  createdAt: '2026-08-06T00:00:00.000Z',
  ...overrides
})

describe('buildWebDavDownloadUrl', () => {
  it('preserves the WebDAV service base path for aria2 downloads', () => {
    const url = new URL(buildWebDavDownloadUrl(connection(), '/Movies/demo.mkv'))

    expect(url.pathname).toBe('/dav/Movies/demo.mkv')
  })

  it('combines the service path, mounted root and encoded nested file path exactly once', () => {
    const url = new URL(buildWebDavDownloadUrl(connection({ url: 'https://example.com/remote.php/dav/files/alice', rootPath: '/媒体库' }), '/电影/天空 之城.mkv'))

    expect(decodeURIComponent(url.pathname)).toBe('/remote.php/dav/files/alice/媒体库/电影/天空 之城.mkv')
    expect(url.pathname).toContain('%E5%A4%A9%E7%A9%BA%20%E4%B9%8B%E5%9F%8E.mkv')
  })

  it('returns the direct authenticated URL for plain WebDAV without probing the AList API', async () => {
    const originalFetch = globalThis.fetch
    globalThis.fetch = (() => { throw new Error('plain WebDAV must not call fetch') }) as typeof fetch
    try {
      const url = new URL(await getWebDavDownloadUrl(connection(), '/Movies/demo.mkv'))
      expect(url.pathname).toBe('/dav/Movies/demo.mkv')
      expect(url.username).toBe('alice')
    } finally {
      globalThis.fetch = originalFetch
    }
  })
})
