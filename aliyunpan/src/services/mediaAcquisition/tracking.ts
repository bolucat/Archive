import type { MediaAcquisitionTrackingItem } from '@shared/types/mediaAcquisition'

export interface TrackingMetadata {
  totalEpisodes: number
  latestAiredEpisode: number
}

export interface TrackingSnapshot extends TrackingMetadata {
  obtainedEpisodeNumbers: number[]
  missingEpisodes: number[]
  status: MediaAcquisitionTrackingItem['status']
}

export function parsePatrolTimes(value: string | string[] | undefined): string[] {
  const source = Array.isArray(value) ? value : String(value || '').split(',')
  const times = [...new Set(source.map(item => item.trim()).filter(item => /^([01]\d|2[0-3]):[0-5]\d$/.test(item)))].sort()
  return times.length ? times.slice(0, 6) : ['06:00', '21:00']
}

export function nextPatrolAt(value: string | string[] | undefined, now = Date.now()): number {
  const offset = 8 * 60 * 60 * 1000
  const beijingNow = new Date(now + offset)
  for (const time of parsePatrolTimes(value)) {
    const [hour, minute] = time.split(':').map(Number)
    const candidate = Date.UTC(beijingNow.getUTCFullYear(), beijingNow.getUTCMonth(), beijingNow.getUTCDate(), hour, minute) - offset
    if (candidate > now) return candidate
  }
  const [hour, minute] = parsePatrolTimes(value)[0].split(':').map(Number)
  return Date.UTC(beijingNow.getUTCFullYear(), beijingNow.getUTCMonth(), beijingNow.getUTCDate() + 1, hour, minute) - offset
}

function aired(date: unknown, now: number): boolean {
  if (typeof date !== 'string' || !date) return false
  const timestamp = Date.parse(date)
  return Number.isFinite(timestamp) && timestamp <= now
}

export function readTrackingMetadata(tv: any, season: any, seasonNumber: number, now = Date.now()): TrackingMetadata {
  const seasonSummary = Array.isArray(tv?.seasons) ? tv.seasons.find((item: any) => Number(item?.season_number) === seasonNumber) : undefined
  const episodes = Array.isArray(season?.episodes) ? season.episodes : []
  const totalEpisodes = Math.max(0, Number(seasonSummary?.episode_count || season?.episode_count || episodes.length || 0))
  const last = tv?.last_episode_to_air
  const lastAired = Number(last?.season_number) === seasonNumber && Number(last?.episode_number) > 0 && (!last?.air_date || aired(last.air_date, now))
    ? Number(last.episode_number)
    : 0
  const datedAired = episodes.reduce((latest: number, episode: any) => aired(episode?.air_date, now) ? Math.max(latest, Number(episode?.episode_number) || 0) : latest, 0)
  return { totalEpisodes: Math.max(totalEpisodes, lastAired, datedAired), latestAiredEpisode: Math.max(lastAired, datedAired) }
}

export function extractObtainedEpisodeNumbers(names: string[], seasonNumber: number): number[] {
  const numbers = new Set<number>()
  const season = String(seasonNumber).padStart(2, '0')
  for (const name of names) {
    const normalized = name.normalize('NFKC')
    const patterns = [
      new RegExp(`S0?${seasonNumber}[ ._\-]*E(?:P)?[ ._\-]*(\\d{1,3})`, 'i'),
      new RegExp(`\\b0?${seasonNumber}x(\\d{1,3})\\b`, 'i'),
      /\bE(?:P)?[ ._\-]*(\d{1,3})\b/i,
      /第[ ._\-]*(\d{1,3})[ ._\-]*(?:集|话|話)/i
    ]
    for (const pattern of patterns) {
      const match = normalized.match(pattern)
      if (!match) continue
      const episode = Number(match[1])
      if (Number.isInteger(episode) && episode > 0) numbers.add(episode)
      break
    }
  }
  return [...numbers].sort((a, b) => a - b)
}

export function buildTrackingSnapshot(existing: Pick<MediaAcquisitionTrackingItem, 'totalEpisodes' | 'latestAiredEpisode' | 'status'> | undefined, metadata: TrackingMetadata, obtainedEpisodeNumbers: number[]): TrackingSnapshot {
  const totalEpisodes = Math.max(existing?.totalEpisodes || 0, metadata.totalEpisodes)
  const latestAiredEpisode = Math.min(totalEpisodes || metadata.latestAiredEpisode, Math.max(existing?.latestAiredEpisode || 0, metadata.latestAiredEpisode))
  const obtained = [...new Set(obtainedEpisodeNumbers)].filter(number => Number.isInteger(number) && number > 0).sort((a, b) => a - b)
  const obtainedSet = new Set(obtained)
  const missingEpisodes = Array.from({ length: latestAiredEpisode }, (_, index) => index + 1).filter(number => !obtainedSet.has(number))
  const status = existing?.status === 'ended'
    ? 'ended'
    : totalEpisodes > 0 && latestAiredEpisode >= totalEpisodes && missingEpisodes.length === 0 ? 'complete' : 'tracking'
  return { totalEpisodes, latestAiredEpisode, obtainedEpisodeNumbers: obtained, missingEpisodes, status }
}
