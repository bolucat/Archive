import { describe, expect, it } from 'vitest'
import { buildTrackingSnapshot, extractObtainedEpisodeNumbers, nextPatrolAt, parsePatrolTimes, readTrackingMetadata } from '../../services/mediaAcquisition/tracking'

describe('media acquisition tracking', () => {
  it('treats only aired episodes as acquisition gaps', () => {
    const metadata = readTrackingMetadata({ seasons: [{ season_number: 1, episode_count: 8 }] }, { episodes: [
      { episode_number: 1, air_date: '2026-01-01' },
      { episode_number: 2, air_date: '2026-01-08' },
      { episode_number: 3, air_date: '2027-01-01' }
    ] }, 1, Date.parse('2026-07-17'))
    const snapshot = buildTrackingSnapshot(undefined, metadata, [1])
    expect(snapshot).toMatchObject({ totalEpisodes: 8, latestAiredEpisode: 2, missingEpisodes: [2], status: 'tracking' })
  })

  it('advances the aired cursor without regressing stale metadata', () => {
    const existing = { totalEpisodes: 10, latestAiredEpisode: 6, status: 'tracking' as const }
    const snapshot = buildTrackingSnapshot(existing, { totalEpisodes: 8, latestAiredEpisode: 4 }, [1, 2, 3, 4, 5, 6])
    expect(snapshot).toMatchObject({ totalEpisodes: 10, latestAiredEpisode: 6, missingEpisodes: [], status: 'tracking' })
  })

  it('marks a season complete only after the finale aired and every episode exists', () => {
    const snapshot = buildTrackingSnapshot(undefined, { totalEpisodes: 3, latestAiredEpisode: 3 }, [1, 2, 3])
    expect(snapshot.status).toBe('complete')
  })

  it('extracts common episode naming formats for the tracked season', () => {
    expect(extractObtainedEpisodeNumbers(['Show.S02E03.mkv', 'Show 2x04.mp4', 'Show 第 5 集.mkv', 'Show.E06.mkv'], 2)).toEqual([3, 4, 5, 6])
  })

  it('schedules the next patrol by Beijing time and validates configured slots', () => {
    expect(parsePatrolTimes('21:00,invalid,06:00,21:00')).toEqual(['06:00', '21:00'])
    expect(new Date(nextPatrolAt('06:00,21:00', Date.parse('2026-07-17T04:00:00Z'))).toISOString()).toBe('2026-07-17T13:00:00.000Z')
    expect(new Date(nextPatrolAt('06:00,21:00', Date.parse('2026-07-17T14:00:00Z'))).toISOString()).toBe('2026-07-17T22:00:00.000Z')
  })
})
