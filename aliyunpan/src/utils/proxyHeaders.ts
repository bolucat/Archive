import type { IncomingHttpHeaders } from 'http'

export type ProxyResponseHeaders = Record<string, string | string[] | number | undefined>

const DROPPED_UPSTREAM_HEADERS = new Set([
  'host',
  'connection',
  'proxy-connection',
  'keep-alive',
  'transfer-encoding',
  'upgrade',
  'te',
  'trailer',
  'referer',
  'authorization',
  'if-none-match',
  'if-modified-since'
])

export function buildUpstreamProxyHeaders(
  incomingHeaders: IncomingHttpHeaders,
  proxyHeaders?: string
): Record<string, string | string[]> {
  const headers: Record<string, string | string[]> = {}

  for (const [key, value] of Object.entries(incomingHeaders)) {
    if (value == null) continue
    const normalizedKey = key.toLowerCase()
    if (DROPPED_UPSTREAM_HEADERS.has(normalizedKey)) continue
    headers[normalizedKey] = value
  }

  headers['accept-encoding'] = 'identity'

  if (proxyHeaders) {
    try {
      const extraHeaders = JSON.parse(String(proxyHeaders)) as Record<string, string>
      for (const [key, value] of Object.entries(extraHeaders || {})) {
        if (!value) continue
        headers[key.toLowerCase()] = value
      }
    } catch (error) {
      console.warn('proxy_headers parse error', error)
    }
  }

  return headers
}

export function ensureInlinePreviewRange(headers: Record<string, string | string[]>, isInlinePreview: boolean): Record<string, string | string[]> {
  if (isInlinePreview && !headers.range) headers.range = 'bytes=0-'
  return headers
}

export function normalizeProxyStatusCode(statusCode: number, contentRange?: string | string[] | number): number {
  return statusCode === 200 && !!contentRange ? 206 : statusCode
}

export function normalizeProxyRangeHeaders(headers: ProxyResponseHeaders): ProxyResponseHeaders {
  const acceptRanges = headers['accept-ranges']
  const values = Array.isArray(acceptRanges) ? acceptRanges : String(acceptRanges || '').split(',').map(value => value.trim()).filter(Boolean)
  if (headers['content-range'] && values.length > 0 && values.every(value => String(value).toLowerCase() === 'bytes')) {
    headers['accept-ranges'] = 'bytes'
  }
  return headers
}
