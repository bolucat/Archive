import { afterEach, describe, expect, it, vi } from 'vitest'
import { tmdbCertification } from '../tmdbCertification'
import { TmdbService } from '../tmdb'

afterEach(() => vi.unstubAllGlobals())
describe('TMDB certifications', () => {
  it('prefers a nonempty US movie rating and falls back when US is empty', () => {
    const data = { release_dates: { results: [
      { iso_3166_1: 'GB', release_dates: [{ certification: '15' }] },
      { iso_3166_1: 'US', release_dates: [{ certification: ' ' }, { certification: 'R' }] }
    ] } }
    expect(tmdbCertification(data, false)).toBe('R')
    data.release_dates.results[1].release_dates.pop()
    expect(tmdbCertification(data, false)).toBe('15')
    expect(tmdbCertification({}, false)).toBeUndefined()
  })
  it('reads TV content ratings without confusing them with community scores', () => {
    expect(tmdbCertification({ rating: 8.5, content_ratings: { results: [{ iso_3166_1: 'US', rating: 'TV-MA' }] } }, true)).toBe('TV-MA')
    expect(tmdbCertification({ rating: 8.5 }, true)).toBeUndefined()
    expect(tmdbCertification({ releases: { countries: [{ iso_3166_1: 'US', certification: 'PG' }] } }, false)).toBe('PG')
  })
  it('supplements metadata omitted by the detail proxy', async () => {
    const fetch = vi.fn()
      .mockResolvedValueOnce({ ok: true, json: async () => ({ data: { id: 42, title: 'Example' } }) })
      .mockResolvedValueOnce({ ok: true, json: async () => ({ results: [{ iso_3166_1: 'US', release_dates: [{ certification: 'PG-13' }] }] }) })
    vi.stubGlobal('fetch', fetch)
    expect((await TmdbService.getInstance().getMovieByTmdbId(42))?.certification).toBe('PG-13')
    expect(fetch.mock.calls[1][0]).toContain('/proxy/movie/42/release_dates')
  })
  it('keeps a successful match when the optional rating request fails', async () => {
    vi.stubGlobal('fetch', vi.fn()
      .mockResolvedValueOnce({ ok: true, json: async () => ({ data: { id: 43, title: 'Example' } }) })
      .mockResolvedValueOnce({ ok: false, status: 404 }))
    expect(await TmdbService.getInstance().getMovieByTmdbId(43)).toMatchObject({ id: 43 })
  })
})
