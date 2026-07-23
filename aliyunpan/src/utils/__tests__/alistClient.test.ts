import { afterEach, describe, expect, it, vi } from 'vitest'
import { createAListConnection, getWebDavDownloadUrl, listWebDavDirectory } from '../webdavClient'

describe('AList media source client', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('logs in, lists the configured media root, and maps AList entries', async () => {
    const fetchMock = vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
      const url = String(input)
      if (url.endsWith('/api/auth/login')) {
        return new Response(JSON.stringify({ code: 200, message: 'success', data: { token: 'alist-token' } }), { status: 200 })
      }
      if (url.endsWith('/api/fs/list')) {
        expect(init?.headers).toMatchObject({ Authorization: 'alist-token' })
        expect(JSON.parse(String(init?.body))).toMatchObject({ path: '/Movies', page: 1, per_page: 0 })
        return new Response(JSON.stringify({
          code: 200,
          message: 'success',
          data: {
            content: [
              { name: 'Spirited Away.mkv', size: 1024, is_dir: false, modified: '2026-07-22T10:00:00Z', thumb: '/thumb.jpg', hash_info: { md5: 'abc' } },
              { name: 'Anime', size: 0, is_dir: true, modified: '2026-07-22T10:00:00Z' }
            ]
          }
        }), { status: 200 })
      }
      throw new Error(`Unexpected request: ${url}`)
    })
    vi.stubGlobal('fetch', fetchMock)

    const connection = createAListConnection({ name: 'Home', url: 'https://alist.example.com/base/', username: 'user', password: 'pass', rootPath: '/Movies' })
    const items = await listWebDavDirectory(connection, '/')

    expect(connection).toMatchObject({ kind: 'alist', url: 'https://alist.example.com/base/dav', rootPath: '/Movies' })
    expect(items).toHaveLength(2)
    expect(items[0]).toMatchObject({ drive_id: `webdav:${connection.id}`, file_id: '/Spirited Away.mkv', parent_file_id: '/', category: 'video', content_hash: 'abc' })
    expect(items[1]).toMatchObject({ file_id: '/Anime', isDir: true, category: 'folder' })
  })

  it('uses the AList file API raw_url for playback', async () => {
    const fetchMock = vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
      const url = String(input)
      if (url.endsWith('/api/auth/login')) {
        return new Response(JSON.stringify({ code: 200, data: { token: 'play-token' } }), { status: 200 })
      }
      if (url.endsWith('/api/fs/get')) {
        expect(JSON.parse(String(init?.body))).toMatchObject({ path: '/Movies/Spirited Away.mkv' })
        return new Response(JSON.stringify({ code: 200, data: { raw_url: 'https://cdn.example.com/video.mkv' } }), { status: 200 })
      }
      throw new Error(`Unexpected request: ${url}`)
    })
    vi.stubGlobal('fetch', fetchMock)

    const connection = createAListConnection({ name: 'Home', url: 'https://alist.example.com', username: 'user', password: 'pass', rootPath: '/Movies' })
    await expect(getWebDavDownloadUrl(connection, '/Spirited Away.mkv')).resolves.toBe('https://cdn.example.com/video.mkv')
  })
})
