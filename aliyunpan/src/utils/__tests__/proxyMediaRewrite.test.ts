import { describe, expect, it } from 'vitest'
import { createMpvProxyContext, resolveMpvProxyUri, rewriteMpvProxyPlaylist } from '../proxyMediaRewrite'

const buildProxyUrl = (target: string, context: ReturnType<typeof createMpvProxyContext>) => {
  const url = new URL('http://127.0.0.1:3884/proxy')
  Object.entries({ ...context, proxy_url: target }).forEach(([key, value]) => {
    if (value) url.searchParams.set(key, String(value))
  })
  return url.href
}

const context = createMpvProxyContext({
  user_id: '115_user',
  drive_id: 'drive115',
  file_id: 'file-id',
  file_size: '123',
  quality: '4',
  proxy_headers: '{"Authorization":"Bearer token"}',
  proxy_kind: 'mpv'
})

const expectProxyTarget = (value: string, target: string) => {
  const url = new URL(value)
  expect(url.origin).toBe('http://127.0.0.1:3884')
  expect(url.pathname).toBe('/proxy')
  expect(url.searchParams.get('user_id')).toBe('115_user')
  expect(url.searchParams.get('drive_id')).toBe('drive115')
  expect(url.searchParams.get('file_id')).toBe('file-id')
  expect(url.searchParams.get('file_size')).toBe('123')
  expect(url.searchParams.get('quality')).toBe('4')
  expect(url.searchParams.get('proxy_headers')).toBe('{"Authorization":"Bearer token"}')
  expect(url.searchParams.get('proxy_kind')).toBe('mpv')
  expect(url.searchParams.get('proxy_url')).toBe(target)
}

describe('resolveMpvProxyUri', () => {
  it('keeps redirects inside the authenticated proxy', () => {
    const result = resolveMpvProxyUri('../media/video.mp4?token=1', 'https://videoplay.115.com/path/master.m3u8', context, buildProxyUrl)

    expectProxyTarget(result, 'https://videoplay.115.com/media/video.mp4?token=1')
  })

  it('does not wrap existing proxy or non-http urls', () => {
    const proxyUrl = buildProxyUrl('https://cdn.115.com/video.ts', context)

    expect(resolveMpvProxyUri(proxyUrl, 'https://videoplay.115.com/master.m3u8', context, buildProxyUrl)).toBe(proxyUrl)
    expect(resolveMpvProxyUri('data:text/plain,key', 'https://videoplay.115.com/master.m3u8', context, buildProxyUrl)).toBe('data:text/plain,key')
  })
})

describe('rewriteMpvProxyPlaylist', () => {
  it('rewrites segments, child playlists and URI attributes while preserving CRLF', () => {
    const source = '#EXTM3U\r\n#EXT-X-KEY:METHOD=AES-128,URI="keys/key.bin"\r\n#EXT-X-MAP:URI="/init.mp4"\r\n#EXT-X-STREAM-INF:BANDWIDTH=1000\r\nchild/index.m3u8\r\n#EXTINF:10,\r\n//cdn.115.com/segment.ts?x=1\r\n'
    const result = rewriteMpvProxyPlaylist(source, 'https://videoplay.115.com/path/master.m3u8', context, buildProxyUrl)
    const lines = result.split('\r\n')
    const keyUrl = lines[1].match(/URI="([^"]+)"/)?.[1] || ''
    const mapUrl = lines[2].match(/URI="([^"]+)"/)?.[1] || ''

    expectProxyTarget(keyUrl, 'https://videoplay.115.com/path/keys/key.bin')
    expectProxyTarget(mapUrl, 'https://videoplay.115.com/init.mp4')
    expectProxyTarget(lines[4], 'https://videoplay.115.com/path/child/index.m3u8')
    expectProxyTarget(lines[6], 'https://cdn.115.com/segment.ts?x=1')
    expect(result.endsWith('\r\n')).toBe(true)
  })
})
