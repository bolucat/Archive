import type { MediaLibraryItem } from '../types/media'

export const LIBRARY_CATEGORY_IDS = ['playlist', 'series', 'recent', 'unwatched', 'daily', 'movies', 'tv', 'other', 'music', 'books'] as const
export const MOVIE_CATEGORY_IDS = ['all', 'recent', 'unwatched', 'genres', 'ratings', 'years', 'certification', 'resolution', 'concerts', 'shorts'] as const
export const TV_CATEGORY_IDS = ['all', 'recent', 'unwatched', 'mini', 'genres', 'resolution'] as const
export type LibraryCategoryId = typeof LIBRARY_CATEGORY_IDS[number]
export const LIBRARY_HOME_ROUTES: Record<string, string> = { recent: 'recent', unwatched: 'local:unwatched', daily: 'local:daily', 'movies/all': 'local:movies', 'movies/recent': 'recent', 'movies/unwatched': 'local:unwatched', 'movies/genres': 'local:genres', 'movies/ratings': 'local:ratings', 'movies/years': 'local:years', 'tv/all': 'local:tv', other: 'local:unmatched', music: 'music', books: 'books' }

export function libraryHomeId(route: string) {
  if (route === 'playlist') return 'local:playlist'
  if (route === 'series') return 'custom-series'
  if (route.startsWith('playlist/')) return 'local:playlist:group:' + route.slice('playlist/'.length)
  return LIBRARY_HOME_ROUTES[route] || 'catalog:' + route
}
export function libraryYearGroup(year: string) { const value = Number(year); return value === new Date().getFullYear() ? year : `${Math.floor(value / 10) * 10}s` }

export function libraryMediaFiles(item: MediaLibraryItem) {
  return item.type === 'tv' ? (item.seasons || []).flatMap(season => season.episodes || []).flatMap(episode => episode.driveFiles || []) : item.driveFiles
}

export function libraryResolution(item: MediaLibraryItem): string[] {
  const heights = libraryMediaFiles(item).map(file => file.height || Number(file.name.match(/\b(2160|1080|720|480)p\b/i)?.[1]) || (/\b4k\b/i.test(file.name) ? 2160 : 0))
  return [...new Set(heights.map(height => height >= 2160 ? '4K' : height >= 1080 ? '1080p' : height >= 720 ? '720p' : height > 0 ? 'SD' : ''))].filter(Boolean)
}

export function filterLibraryCategory(items: MediaLibraryItem[], route: string, watched: (item: MediaLibraryItem) => boolean): MediaLibraryItem[] {
  const [root, category = 'all', rawValue = ''] = route.split('/')
  const value = decodeURIComponent(rawValue)
  let result = items.filter(item => root === 'movies' ? item.type === 'movie' : root === 'tv' ? item.type === 'tv' : root === 'other' ? item.type === 'unmatched' : true)
  if (root === 'unwatched' || category === 'unwatched') result = result.filter(item => !watched(item))
  if (category === 'genres' && value) result = result.filter(item => item.genres.includes(value))
  if (category === 'ratings' && value) result = result.filter(item => Math.floor(item.rating ?? -1).toString() === value)
  if (category === 'years' && value) result = result.filter(item => item.year === value || libraryYearGroup(item.year || '') === value)
  if (category === 'resolution' && value) result = result.filter(item => libraryResolution(item).includes(value))
  if (category === 'certification' && value) result = result.filter(item => item.certification === value)
  // Unknown source metadata must not be guessed from the title or season count.
  if (category === 'mini') result = result.filter(item => item.isMiniseries === true)
  if (category === 'concerts') result = result.filter(item => item.mediaSubtype === 'concert')
  if (category === 'shorts') result = result.filter(item => item.mediaSubtype === 'short')
  if (category === 'movies') result = result.filter(item => !item.mediaSubtype || item.mediaSubtype === 'movie')
  if (root === 'recent' || category === 'recent') result = [...result].sort((a, b) => new Date(b.addedAt).getTime() - new Date(a.addedAt).getTime())
  return result
}

export function libraryCategoryGroups(items: MediaLibraryItem[], category: string) {
  const groups = new Map<string, MediaLibraryItem[]>()
  for (const item of items) {
    const values = category === 'genres' ? item.genres : category === 'ratings' ? (item.rating === undefined ? [] : [String(Math.floor(item.rating))]) : category === 'years' ? (item.year ? [libraryYearGroup(item.year)] : []) : category === 'resolution' ? libraryResolution(item) : category === 'certification' ? (item.certification ? [item.certification] : []) : []
    for (const value of values) groups.set(value, [...(groups.get(value) || []), item])
  }
  return [...groups].map(([name, members]) => ({ name, members })).sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }))
}

export function libraryMovieSeries(items: MediaLibraryItem[]) {
  const groups = new Map<string, { id: string; title: string; members: MediaLibraryItem[] }>()
  for (const item of items.filter(item => item.type === 'movie' && item.collectionId)) {
    const id = String(item.collectionId)
    const group = groups.get(id) || { id, title: item.collectionName || item.name, members: [] }
    // A collection may already be stored as one aggregate item with its movies nested.
    const members = item.collectionMovies?.length ? item.collectionMovies : [item]
    for (const member of members) if (!group.members.some(existing => existing.id === member.id)) group.members.push(member)
    groups.set(id, group)
  }
  return [...groups.values()]
}

export function libraryPlaylistMembers(items: MediaLibraryItem[], ids: string[]): MediaLibraryItem[] {
  return ids.flatMap(id => {
    const exact = items.flatMap(item => [item, ...(item.collectionMovies || [])]).find(item => item.id === id)
    if (exact) return [exact]
    for (const series of items.filter(item => item.type === 'tv')) {
      const episode = series.seasons?.flatMap(season => season.episodes || []).find(episode => `${series.id}_${episode.seasonNumber}_${episode.episodeNumber}` === id)
      if (episode) return [{ ...series, id, name: `${series.name} · S${episode.seasonNumber}E${episode.episodeNumber} ${episode.name}`, type: 'unmatched' as const, posterUrl: episode.stillPath || series.posterUrl, driveFiles: episode.driveFiles }]
    }
    return []
  })
}
