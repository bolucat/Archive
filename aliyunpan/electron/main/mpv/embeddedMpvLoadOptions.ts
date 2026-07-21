import type { EmbeddedMpvLoadRequest } from './embeddedMpvBridge'

export function buildMpvLoadOptions(request: EmbeddedMpvLoadRequest): string {
  const options: string[] = []

  const headers = Object.entries(request.headers || {})
    .filter(([key, value]) => key && value != null && String(value))
  const userAgent = headers.find(([key]) => key.toLowerCase() === 'user-agent')?.[1]
  const referrer = headers.find(([key]) => key.toLowerCase() === 'referer' || key.toLowerCase() === 'referrer')?.[1]
  const headerFields = headers
    .filter(([key]) => !['user-agent', 'referer', 'referrer'].includes(key.toLowerCase()))
    .map(([key, value]) => `${key}: ${String(value)}`)

  if (userAgent) options.push(`user-agent=${String(userAgent)}`)
  if (referrer) options.push(`referrer=${String(referrer)}`)
  if (headerFields.length) options.push(`http-header-fields=${headerFields.join(',')}`)
  return options.join(',')
}
