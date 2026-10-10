import { describe, expect, it } from 'vitest'
import type { MediaLibraryItem } from '../../types/media'
import { LIBRARY_CATEGORY_IDS, MOVIE_CATEGORY_IDS, TV_CATEGORY_IDS, filterLibraryCategory, libraryCategoryGroups, libraryMovieSeries, libraryResolution, libraryPlaylistMembers } from '../unifiedLibraryCategories'
const movie = (id: string, extra: Partial<MediaLibraryItem> = {}): MediaLibraryItem => ({ id, parentId: '', type: 'movie', name: id, genres: ['Drama'], driveFiles: [], addedAt: new Date('2026-01-01'), ...extra })
describe('unified library category navigation', () => {
  it('keeps eight video categories plus music and books in order', () => {
    expect(LIBRARY_CATEGORY_IDS).toEqual(['playlist', 'series', 'recent', 'unwatched', 'daily', 'movies', 'tv', 'other', 'music', 'books'])
    expect(MOVIE_CATEGORY_IDS).toHaveLength(10)
    expect(TV_CATEGORY_IDS).toHaveLength(6)
  })
  it('never leaks TV shows into movie subcategories', () => {
    const items = [movie('film'), movie('show', { type: 'tv' })]
    expect(filterLibraryCategory(items, 'movies/genres/Drama', () => false).map(item => item.id)).toEqual(['film'])
    expect(filterLibraryCategory(items, 'tv/unwatched', item => item.id === 'show')).toEqual([])
  })
  it('uses all records rather than the homepage preview limit', () => {
    expect(filterLibraryCategory(Array.from({ length: 75 }, (_, i) => movie(String(i))), 'movies/all', () => false)).toHaveLength(75)
  })
  it('matches ratings, years, and real certification metadata', () => {
    const items = [movie('a', { rating: 7.6, year: '2022', certification: 'PG-13' }), movie('b')]
    for (const route of ['movies/ratings/7', 'movies/years/2022', 'movies/certification/PG-13']) expect(filterLibraryCategory(items, route, () => false).map(item => item.id)).toEqual(['a'])
    expect(libraryCategoryGroups(items, 'certification').map(group => group.name)).toEqual(['PG-13'])
  })
  it('does not infer a miniseries or concert from its title', () => {
    expect(filterLibraryCategory([movie('Concert')], 'movies/concerts', () => false)).toEqual([])
    expect(filterLibraryCategory([movie('limited', { type: 'tv', isMiniseries: true })], 'tv/mini', () => false)).toHaveLength(1)
  })
  it('groups nested and standalone collection members without duplicates', () => {
    const a = { ...movie('a'), type: 'movie' as const }, b = { ...movie('b'), type: 'movie' as const }
    expect(libraryMovieSeries([movie('root', { collectionId: 1, collectionName: 'Saga', collectionMovies: [a, b] }), movie('a', { collectionId: 1 })])[0].members.map(item => item.id)).toEqual(['a', 'b'])
  })
  it('recognizes filename resolution without assigning unknown files to SD', () => {
    expect(libraryResolution(movie('a', { driveFiles: [{ id: 'f', name: 'movie.1080p.mkv', driveId: 'local', driveServerId: '', path: '', fileSize: 0 }] }))).toEqual(['1080p'])
    expect(libraryResolution(movie('unknown'))).toEqual([])
  })
  it('preserves playlist episode identity and order without playing episode one instead', () => {
    const items = [movie('show', { type: 'tv', seasons: [{ id: 1, seasonNumber: 1, name: '', episodeCount: 2, episodes: [{ id: 2, episodeNumber: 2, seasonNumber: 1, name: 'Second', driveFiles: [] }] }] }), movie('film')]
    const members = libraryPlaylistMembers(items, ['show_1_2', 'film', 'missing'])
    expect(members.map(item => item.id)).toEqual(['show_1_2', 'film'])
    expect(members[0].name).toContain('S1E2')
  })
})
