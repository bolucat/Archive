import { describe, expect, it } from 'vitest'
import { MediaFileNormalizer } from '../mediaFileNormalizer'

describe('MediaFileNormalizer', () => {
  const normalizer = new MediaFileNormalizer()

  it('returns one descriptor for release-style TV names', () => {
    expect(normalizer.normalize('Show.Name.S02E03.1080p.WEB-DL.mkv')).toEqual({
      normalizedFileName: 'show.name.s02e03.1080p.web-dl',
      cleanedTitle: 'show name',
      searchTitle: 'show name',
      releaseYear: undefined,
      seasonNumber: 2,
      episodeNumber: 3
    })
  })

  it('extracts movie title and release year before technical tags', () => {
    const result = normalizer.normalize('Movie.Title.2024.2160p.mkv')
    expect(result.cleanedTitle).toBe('movie title')
    expect(result.searchTitle).toBe('movie title')
    expect(result.releaseYear).toBe(2024)
    expect(result.seasonNumber).toBeUndefined()
  })

  it('does not mistake titles for episode markers or discard sequel numbers', () => {
    expect(normalizer.normalize('Se7en.1995.mkv')).toMatchObject({
      searchTitle: 'se7en 1995',
      seasonNumber: undefined,
      episodeNumber: undefined
    })
    expect(normalizer.normalize('1917.mkv')).toMatchObject({ cleanedTitle: '1917', searchTitle: undefined })
    expect(normalizer.normalize('流浪地球 2 2023.mkv').searchTitle).toBe('流浪地球 2 2023')
  })

  it('supports alternate, numeric and Chinese episode formats', () => {
    expect(normalizer.normalize('Show.S01_02.1080p.mkv')).toMatchObject({ searchTitle: 'show', seasonNumber: 1, episodeNumber: 2 })
    expect(normalizer.normalize('Show.1x03.WEBRip.mkv')).toMatchObject({ searchTitle: 'show', seasonNumber: 1, episodeNumber: 3 })
    expect(normalizer.normalize('庆余年.第二季.第十话.1080p.mkv')).toMatchObject({ searchTitle: '庆余年', seasonNumber: 2, episodeNumber: 10 })
  })

  it('uses folder context for numeric and marker-only episode names', () => {
    expect(normalizer.normalize('03.mkv', '/Library/My Show/Season 02/03.mkv')).toMatchObject({
      searchTitle: 'my show',
      seasonNumber: 2,
      episodeNumber: 3
    })
    expect(normalizer.normalize('S01E01.mkv', '/Library/My Show/Season 01/S01E01.mkv')).toMatchObject({
      searchTitle: 'my show',
      seasonNumber: 1,
      episodeNumber: 1
    })
  })
})
