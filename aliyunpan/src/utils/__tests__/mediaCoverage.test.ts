import { describe, expect, it } from 'vitest'
import type { MediaLibraryItem } from '../../types/media'
import { buildExpectedSeasons, getMediaCoverage } from '../mediaCoverage'

function item(overrides: Partial<MediaLibraryItem> = {}): MediaLibraryItem {
  return {
    id: 'tv-1', parentId: 'root', type: 'tv', name: 'Test', genres: [], driveFiles: [], addedAt: new Date(),
    expectedSeasons: [
      { seasonNumber: 1, episodeCount: 3, airDate: '2024-01-01' },
      { seasonNumber: 2, episodeCount: 2, airDate: '2025-01-01' },
      { seasonNumber: 3, episodeCount: 1, airDate: '2099-01-01' }
    ],
    seasons: [{ id: 1, seasonNumber: 1, name: 'S1', episodeCount: 3, episodes: [
      { id: 1, seasonNumber: 1, episodeNumber: 1, name: 'E1', driveFiles: [] },
      { id: 3, seasonNumber: 1, episodeNumber: 3, name: 'E3', driveFiles: [] }
    ] }],
    ...overrides
  }
}

describe('getMediaCoverage', () => {
  it('finds missing seasons and episodes but ignores unaired seasons', () => {
    const coverage = getMediaCoverage(item(), new Date('2026-01-01'))
    expect(coverage?.missingSeasons).toEqual([2])
    expect(coverage?.seasonGaps).toEqual([
      { seasonNumber: 1, missingEpisodes: [2] },
      { seasonNumber: 2, missingEpisodes: [1, 2] }
    ])
    expect(coverage?.summary).toBe('缺 1 季 · 缺 3 集')
  })

  it('uses episode air dates when available', () => {
    const coverage = getMediaCoverage(item({
      expectedSeasons: [{ seasonNumber: 1, episodeCount: 3, episodes: [
        { episodeNumber: 1, airDate: '2025-01-01' },
        { episodeNumber: 2, airDate: '2027-01-01' },
        { episodeNumber: 3, airDate: '2027-01-08' }
      ] }],
      seasons: []
    }), new Date('2026-01-01'))
    expect(coverage?.seasonGaps).toEqual([{ seasonNumber: 1, missingEpisodes: [1] }])
  })

  it('uses the TMDB latest-aired cursor for seasons without episode details', () => {
    const expected = buildExpectedSeasons({
      tv: {
        last_episode_to_air: { season_number: 2, episode_number: 3 },
        seasons: [{ season_number: 1, episode_count: 8 }, { season_number: 2, episode_count: 10 }, { season_number: 3, episode_count: 6 }]
      }
    })
    expect(expected.map(season => [season.seasonNumber, season.latestAiredEpisode])).toEqual([[1, 8], [2, 3], [3, 0]])
  })
})
