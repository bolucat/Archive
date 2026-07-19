import type { MediaAcquisitionFileSnapshot, MediaAcquisitionTarget } from '@shared/types/mediaAcquisition'

const VIDEO_EXT = /\.(mp4|mkv|avi|mov|wmv|flv|webm|m4v|mpg|mpeg|3gp|rmvb|ts|m2ts|mts|vob)$/i

export interface MediaAcquisitionDuplicateGroup {
  episode: number
  keep: MediaAcquisitionFileSnapshot
  deleteCandidates: MediaAcquisitionFileSnapshot[]
}

export function selectRequestedEpisodeFiles(target: Pick<MediaAcquisitionTarget, 'mediaType' | 'seasonNumber' | 'missingEpisodes'>, files: MediaAcquisitionFileSnapshot[]): MediaAcquisitionFileSnapshot[] {
  if (!['tv', 'anime'].includes(target.mediaType) || !target.missingEpisodes?.length) return files
  const requested = new Set(target.missingEpisodes)
  const matched = files.filter(file => {
    const episode = extractMediaAcquisitionEpisodeNumber(file.name, target.seasonNumber || 1)
    return episode !== undefined && requested.has(episode)
  })
  return matched.length ? matched : files
}

export function extractMediaAcquisitionEpisodeNumber(name: string, seasonNumber = 1): number | undefined {
  const normalized = name.normalize('NFKC')
  const season = String(seasonNumber).padStart(2, '0')
  const patterns = [
    new RegExp(`S0?${seasonNumber}[ ._\\-]*E(?:P)?[ ._\\-]*(\\d{1,3})`, 'i'),
    new RegExp(`\\b0?${seasonNumber}x(\\d{1,3})\\b`, 'i'),
    new RegExp(`第\\s*${seasonNumber}\\s*季\\s*第\\s*(\\d{1,3})\\s*(?:集|话|話)`, 'i'),
    /\bE(?:P)?[ ._\-]*(\d{1,3})\b/i,
    /第\s*(\d{1,3})\s*(?:集|话|話)/i
  ]
  for (const pattern of patterns) {
    const match = normalized.match(pattern)
    if (!match) continue
    const episode = Number(match[1])
    if (Number.isInteger(episode) && episode > 0 && season) return episode
  }
  return undefined
}

export function findMediaAcquisitionDuplicateEpisodes(target: Pick<MediaAcquisitionTarget, 'mediaType' | 'seasonNumber'>, files: MediaAcquisitionFileSnapshot[]): MediaAcquisitionDuplicateGroup[] {
  if (!['tv', 'anime'].includes(target.mediaType)) return []
  const seasonNumber = target.seasonNumber || 1
  const grouped = new Map<number, MediaAcquisitionFileSnapshot[]>()
  for (const file of files) {
    if (!VIDEO_EXT.test(file.name)) continue
    const episode = extractMediaAcquisitionEpisodeNumber(file.name, seasonNumber)
    if (!episode) continue
    const group = grouped.get(episode) || []
    group.push(file)
    grouped.set(episode, group)
  }
  return [...grouped.entries()].flatMap(([episode, group]) => {
    if (group.length < 2) return []
    const sorted = [...group].sort((left, right) => Number(right.size || 0) - Number(left.size || 0))
    return [{ episode, keep: sorted[0], deleteCandidates: sorted.slice(1) }]
  }).sort((left, right) => left.episode - right.episode)
}
