export interface MpvProxyContext {
  user_id: string
  drive_id: string
  file_id: string
  file_size: string
  quality: string
  proxy_headers: string
  proxy_kind: 'mpv'
}

export type MpvProxyUrlBuilder = (target: string, context: MpvProxyContext) => string

export function createMpvProxyContext(values: Record<string, unknown>): MpvProxyContext {
  return {
    user_id: String(values.user_id || ''),
    drive_id: String(values.drive_id || ''),
    file_id: String(values.file_id || ''),
    file_size: String(values.file_size || ''),
    quality: String(values.quality || ''),
    proxy_headers: String(values.proxy_headers || ''),
    proxy_kind: 'mpv'
  }
}

export function resolveMpvProxyUri(uri: string, baseUrl: string, context: MpvProxyContext, buildProxyUrl: MpvProxyUrlBuilder) {
  const value = String(uri || '').trim()
  if (!value) return value
  try {
    const resolved = new URL(value, baseUrl)
    if (!['http:', 'https:'].includes(resolved.protocol)) return value
    if (resolved.hostname === '127.0.0.1' && resolved.pathname === '/proxy' && resolved.searchParams.get('proxy_kind') === 'mpv') return value
    return buildProxyUrl(resolved.href, context)
  } catch {
    return value
  }
}

export function rewriteMpvProxyPlaylist(source: string, playlistUrl: string, context: MpvProxyContext, buildProxyUrl: MpvProxyUrlBuilder) {
  return source
    .replace(/URI="([^"]+)"/g, (_match, uri) => `URI="${resolveMpvProxyUri(uri, playlistUrl, context, buildProxyUrl)}"`)
    .split(/(\r?\n)/)
    .map(part => {
      const value = part.trim()
      if (!value || value.startsWith('#') || part === '\n' || part === '\r\n') return part
      const start = part.indexOf(value)
      return `${part.slice(0, start)}${resolveMpvProxyUri(value, playlistUrl, context, buildProxyUrl)}${part.slice(start + value.length)}`
    })
    .join('')
}

export function isM3u8Response(url: string, contentType: string | string[] | undefined) {
  const normalizedType = Array.isArray(contentType) ? contentType.join(';') : String(contentType || '')
  if (/mpegurl/i.test(normalizedType)) return true
  try {
    const parsed = new URL(url)
    return parsed.pathname.toLowerCase().endsWith('.m3u8') || parsed.pathname.toLowerCase().includes('/m3u8/')
  } catch {
    return false
  }
}
