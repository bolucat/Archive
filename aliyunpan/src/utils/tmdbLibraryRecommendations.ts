import type { MediaLibraryItem } from '../types/media'
export interface TmdbRankedEntry { id: number; type: 'movie' | 'tv' }
export function matchLibraryRanking(items: MediaLibraryItem[], ranking: TmdbRankedEntry[]): MediaLibraryItem[] {
  const ranks = new Map(ranking.map((entry, index) => [entry.type + ':' + entry.id, index]))
  const matched = items.flatMap(item => {
    if (item.type !== 'movie' && item.type !== 'tv') return []
    const ids = item.collectionMovies?.length ? item.collectionMovies.map(movie => movie.tmdbId) : [item.tmdbId]
    const positions = ids.flatMap(id => id && ranks.has(item.type + ':' + id) ? [ranks.get(item.type + ':' + id)!] : [])
    return positions.length ? [{ item, rank: Math.min(...positions) }] : []
  }).sort((a, b) => a.rank - b.rank)
  const seen = new Set<string>()
  return matched.flatMap(({item}) => { if (seen.has(item.id)) return []; seen.add(item.id); return [item] })
}
export async function fetchRankingPages(base: string, endpoint: string, type: 'movie' | 'tv', request: (url: string) => Promise<Response>, maxPages = 5): Promise<TmdbRankedEntry[]> {
  const results: TmdbRankedEntry[] = []
  const seen = new Set<number>()
  for (let page = 1; page <= maxPages; page++) {
    const response = await request(base + endpoint + '?language=zh-CN&page=' + page)
    if (!response.ok) throw new Error('TMDB ranking HTTP ' + response.status)
    const payload = await response.json()
    const data = payload?.data?.results ? payload.data : payload
    if (!Array.isArray(data?.results)) throw new Error('Invalid TMDB ranking response')
    for (const entry of data.results) { const id = Number(entry.id); if (Number.isSafeInteger(id) && id > 0 && !seen.has(id)) { seen.add(id); results.push({id, type}) } }
    if (!data.results.length || page >= Number(data.total_pages || 1)) break
  }
  return results
}
