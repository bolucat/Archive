export type Drive115PlaybackAuth = {
  authorization: string
  userAgent: string
}

type Drive115PlaybackAuthEntry = Drive115PlaybackAuth & {
  prefixes: string[]
  expiresAt: number
}

const DEFAULT_TTL_MS = 6 * 60 * 60 * 1000
const MAX_ENTRIES = 256

const buildPrefixes = (rawUrl: string) => {
  try {
    const url = new URL(rawUrl)
    if (url.protocol !== 'http:' && url.protocol !== 'https:') return []
    const path = url.pathname || '/'
    const slash = path.lastIndexOf('/')
    const directory = slash >= 0 ? path.slice(0, slash + 1) : '/'
    return Array.from(new Set([`${url.origin}${path}`, `${url.origin}${directory}`]))
  } catch {
    return []
  }
}

export class Drive115PlaybackAuthRegistry {
  private entries: Drive115PlaybackAuthEntry[] = []

  register(urls: string[], auth: Drive115PlaybackAuth, expiresAt = Date.now() + DEFAULT_TTL_MS) {
    const prefixes = Array.from(new Set(urls.flatMap(buildPrefixes)))
    if (!prefixes.length || !auth.authorization || !auth.userAgent) return
    const now = Date.now()
    this.entries = this.entries.filter(entry => entry.expiresAt > now && !entry.prefixes.some(prefix => prefixes.includes(prefix)))
    this.entries.push({ ...auth, prefixes, expiresAt: Math.max(expiresAt, now + 60_000) })
    if (this.entries.length > MAX_ENTRIES) this.entries.splice(0, this.entries.length - MAX_ENTRIES)
  }

  resolve(rawUrl: string, now = Date.now()): Drive115PlaybackAuth | undefined {
    this.entries = this.entries.filter(entry => entry.expiresAt > now)
    let match: { auth: Drive115PlaybackAuth; length: number } | undefined
    for (const entry of this.entries) {
      for (const prefix of entry.prefixes) {
        if (rawUrl.startsWith(prefix) && (!match || prefix.length > match.length)) {
          match = { auth: { authorization: entry.authorization, userAgent: entry.userAgent }, length: prefix.length }
        }
      }
    }
    return match?.auth
  }
}
