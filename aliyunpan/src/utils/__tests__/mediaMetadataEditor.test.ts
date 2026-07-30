import { describe, expect, it } from 'vitest'
import type { MediaLibraryItem, MediaLibraryTvSeriesItem, MovieItem } from '../../types/media'
import { applyManualMediaMetadataEdit, applyTmdbMovieMatch, applyTmdbTvEpisodeMatch, applyTmdbTvSeasonMatch, applyTmdbTvWholeMatch, preserveManualMediaMetadata } from '../mediaMetadataEditor'

const createTvItem = (): MediaLibraryItem => ({
  id: 'tv-1',
  parentId: 'root',
  type: 'tv',
  name: 'Original series',
  genres: ['Drama'],
  driveFiles: [{ drive_id: 'drive', file_id: 'series-file', name: 'series.mkv' } as any],
  addedAt: new Date('2026-01-01'),
  seasons: [{
    id: 101,
    seasonNumber: 1,
    name: 'Season 1',
    episodeCount: 1,
    episodes: [{
      id: 1001,
      seasonNumber: 1,
      episodeNumber: 1,
      name: 'Episode 1',
      overview: 'Original overview',
      driveFiles: [{ drive_id: 'drive', file_id: 'episode-file', name: 'S01E01.mkv' } as any]
    }]
  }]
})

const createTvDetail = (seasonNumber = 1): MediaLibraryTvSeriesItem => ({
  id: 500,
  tv: {
    id: 500,
    name: 'Correct series',
    first_air_date: '2024-01-02',
    overview: 'Correct series overview',
    vote_average: 8.6,
    poster_path: '/series.jpg',
    genres: [{ id: 18, name: 'Drama' }],
    seasons: [{ id: 600 + seasonNumber, season_number: seasonNumber, name: `Season ${seasonNumber}`, episode_count: 2 }]
  },
  current_season: {
    id: 600 + seasonNumber,
    season_number: seasonNumber,
    name: `Correct season ${seasonNumber}`,
    episode_count: 2,
    episodes: [
      { id: 700 + seasonNumber, season_number: seasonNumber, episode_number: 1, name: `Correct S${seasonNumber}E1`, overview: 'First episode' },
      { id: 800 + seasonNumber, season_number: seasonNumber, episode_number: 2, name: `Correct S${seasonNumber}E2`, overview: 'Second episode' }
    ]
  }
})

describe('media metadata editor', () => {
  it('edits a single episode without changing its season or series metadata', () => {
    const result = applyManualMediaMetadataEdit(createTvItem(), {
      scope: 'episode',
      seasonNumber: 1,
      episodeNumber: 1,
      changes: { name: 'Correct episode', overview: 'Correct overview', runtime: 48 }
    }, 123)

    expect(result.name).toBe('Original series')
    expect(result.seasons?.[0].name).toBe('Season 1')
    expect(result.seasons?.[0].episodes?.[0]).toMatchObject({
      name: 'Correct episode',
      overview: 'Correct overview',
      runtime: 48,
      metadataSource: 'manual',
      metadataUpdatedAt: 123
    })
  })

  it('preserves manually edited series, season and episode fields during a later scrape merge', () => {
    let existing = applyManualMediaMetadataEdit(createTvItem(), { scope: 'item', changes: { name: 'Correct series', year: '2024' } }, 100)
    existing = applyManualMediaMetadataEdit(existing, { scope: 'season', seasonNumber: 1, changes: { name: 'Correct season' } }, 101)
    existing = applyManualMediaMetadataEdit(existing, { scope: 'episode', seasonNumber: 1, episodeNumber: 1, changes: { name: 'Correct episode' } }, 102)

    const incoming = createTvItem()
    incoming.name = 'Wrong scraped series'
    incoming.year = '2025'
    incoming.seasons![0].name = 'Wrong scraped season'
    incoming.seasons![0].episodes![0].name = 'Wrong scraped episode'

    const result = preserveManualMediaMetadata(existing, incoming)
    expect(result.name).toBe('Correct series')
    expect(result.year).toBe('2024')
    expect(result.seasons?.[0].name).toBe('Correct season')
    expect(result.seasons?.[0].episodes?.[0].name).toBe('Correct episode')
  })

  it('preserves a manually edited movie inside a collection', () => {
    const movie = { id: 'movie-1', parentId: 'root', type: 'movie' as const, name: 'Correct movie', genres: [], driveFiles: [], metadataSource: 'manual' as const, metadataUpdatedAt: 200, addedAt: new Date() }
    const existing: MediaLibraryItem = { ...createTvItem(), type: 'movie', collectionMovies: [movie] }
    const incoming: MediaLibraryItem = {
      ...existing,
      collectionMovies: [{ ...movie, name: 'Wrong scraped movie', metadataSource: 'tmdb', metadataUpdatedAt: undefined }]
    }

    expect(preserveManualMediaMetadata(existing, incoming).collectionMovies?.[0].name).toBe('Correct movie')
  })

  it('applies a selected TMDB movie without changing its cloud file binding', () => {
    const item = createTvItem()
    const movie: MovieItem = {
      id: 99,
      title: 'Correct movie',
      original_title: 'Correct movie',
      original_language: 'en',
      overview: 'Correct overview',
      release_date: '2024-04-05',
      vote_average: 8.1,
      poster_path: '/movie.jpg',
      credits: { cast: [], crew: [] }
    }

    const result = applyTmdbMovieMatch(item, movie, 300)
    expect(result).toMatchObject({ type: 'movie', name: 'Correct movie', year: '2024', tmdbId: 99, metadataSource: 'manual', metadataUpdatedAt: 300 })
    expect(result.driveFiles).toBe(item.driveFiles)
  })

  it('applies a whole TV match only to local episodes and preserves their cloud files', () => {
    const item = createTvItem()
    const episodeFiles = item.seasons![0].episodes![0].driveFiles
    const result = applyTmdbTvWholeMatch(item, [createTvDetail(1)], 301)

    expect(result).toMatchObject({ name: 'Correct series', year: '2024', tmdbId: 500, metadataSource: 'manual' })
    expect(result.driveFiles).toBe(item.driveFiles)
    expect(result.seasons![0].name).toBe('Correct season 1')
    expect(result.seasons![0].episodes).toHaveLength(1)
    expect(result.seasons![0].episodes![0].name).toBe('Correct S1E1')
    expect(result.seasons![0].episodes![0].driveFiles).toBe(episodeFiles)
  })

  it('maps a selected TMDB season onto a different local season without renumbering local files', () => {
    const item = createTvItem()
    const episodeFiles = item.seasons![0].episodes![0].driveFiles
    const result = applyTmdbTvSeasonMatch(item, 1, createTvDetail(2), 302)

    expect(result.seasons![0]).toMatchObject({ seasonNumber: 1, name: 'Correct season 2', metadataSource: 'manual' })
    expect(result.seasons![0].episodes![0]).toMatchObject({ seasonNumber: 1, episodeNumber: 1, name: 'Correct S2E1' })
    expect(result.seasons![0].episodes![0].driveFiles).toBe(episodeFiles)
  })

  it('maps a selected TMDB episode onto a different local episode without renumbering it', () => {
    const item = createTvItem()
    const episodeFiles = item.seasons![0].episodes![0].driveFiles
    const result = applyTmdbTvEpisodeMatch(item, 1, 1, createTvDetail(2), 2, 303)

    expect(result.seasons![0].episodes![0]).toMatchObject({ seasonNumber: 1, episodeNumber: 1, name: 'Correct S2E2', metadataSource: 'manual', metadataUpdatedAt: 303 })
    expect(result.seasons![0].episodes![0].driveFiles).toBe(episodeFiles)
  })
})
