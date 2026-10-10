export interface TraktIdentity {
  type: 'movie' | 'tv' | 'episode'
  tmdbId?: number
  imdbId?: string
}
export interface TraktStatus { configured: boolean; connected: boolean; username?: string }
export interface TraktAccount { username: string; name: string; avatar?: string; vip: boolean; episodes: number; shows: number; movies: number }
export interface TraktDeviceLogin { userCode: string; verificationUrl: string; expiresAt: number; interval: number }
export type TraktPollResult = { state: 'pending'; interval: number } | { state: 'connected'; status: TraktStatus }

// Never infer external metadata IDs from a provider file ID or local library ID.
export function traktPayload(identity: TraktIdentity, rating?: number) {
  if (!identity || !['movie', 'tv', 'episode'].includes(identity.type)) throw new Error('TRAKT_METADATA')
  const ids: { tmdb?: number; imdb?: string } = {}
  if (Number.isSafeInteger(identity.tmdbId) && identity.tmdbId! > 0) ids.tmdb = identity.tmdbId
  if (typeof identity.imdbId === 'string' && /^tt\d+$/.test(identity.imdbId)) ids.imdb = identity.imdbId
  if (!Object.keys(ids).length) throw new Error('TRAKT_METADATA')
  if (rating !== undefined && (!Number.isInteger(rating) || rating < 1 || rating > 10)) throw new Error('TRAKT_RATING')
  return { [identity.type === 'movie' ? 'movies' : identity.type === 'episode' ? 'episodes' : 'shows']: [{ ids, ...(rating === undefined ? {} : { rating }) }] }
}
