import type { MediaServerCardItem } from '../types/mediaServerContent'

export const mediaServerSortOptions = ['premiereDate', 'title', 'createdAt', 'rating', 'criticRating', 'runtimeMinutes', 'year', 'playCount', 'sortName', 'addedAt', 'random', 'videoBitrate', 'airTime', 'studio', 'artist', 'officialRating', 'seriesTitle', 'seriesDatePlayed', 'airOrder'] as const
export type MediaServerBrowseSort = typeof mediaServerSortOptions[number]
export type MediaServerSortDirection = 'ascending' | 'descending'

// Stable per visit, so reactive updates/pagination do not reshuffle existing items.
export function randomSortRank(id: string, seed: number): number {
  let hash = seed | 0
  for (let index = 0; index < id.length; index++) hash = Math.imul(hash ^ id.charCodeAt(index), 16777619)
  hash ^= hash >>> 16
  hash = Math.imul(hash, 0x45d9f3b)
  return (hash ^ (hash >>> 16)) >>> 0
}

export function compareMediaServerItems(a: MediaServerCardItem, b: MediaServerCardItem, sort: MediaServerBrowseSort, direction: MediaServerSortDirection, randomSeed = 0): number {
  const value = (item: MediaServerCardItem): string | number | undefined => {
    if (sort === 'random') return randomSortRank(`${item.serverId}:${item.id}`, randomSeed)
    if (sort === 'title') return item.title
    if (sort === 'sortName') return item.sortName || item.title
    if (sort === 'seriesTitle') return item.seriesTitle || item.parentTitle
    if (sort === 'airOrder') return item.seasonNumber === undefined || item.episodeNumber === undefined ? undefined : item.seasonNumber * 100000 + item.episodeNumber
    if (sort === 'premiereDate' || sort === 'createdAt' || sort === 'addedAt' || sort === 'seriesDatePlayed') {
      const raw = item[sort]
      if (raw === undefined || raw === '') return undefined
      const timestamp = new Date(raw).getTime()
      return Number.isFinite(timestamp) ? timestamp : undefined
    }
    return item[sort]
  }
  const x = value(a), y = value(b)
  const missingX = x === undefined || x === '', missingY = y === undefined || y === ''
  if (missingX !== missingY) return missingX ? 1 : -1
  const result = missingX ? 0 : typeof x === 'number' && typeof y === 'number' ? x - y : String(x).localeCompare(String(y), undefined, { numeric: true })
  return (direction === 'descending' ? -result : result) || a.title.localeCompare(b.title, undefined, { numeric: true }) || a.id.localeCompare(b.id)
}
