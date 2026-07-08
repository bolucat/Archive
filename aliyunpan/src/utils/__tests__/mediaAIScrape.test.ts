import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { DriveFileItem, MediaLibraryItem, MovieItem, MediaLibraryTvSeriesItem } from '../../types/media'

vi.mock('../debuglog', () => ({
  default: {
    mSaveWarning: vi.fn(),
    mSaveLog: vi.fn()
  }
}))

vi.mock('../message', () => ({
  default: {
    error: vi.fn(),
    success: vi.fn(),
    warning: vi.fn(),
    info: vi.fn()
  }
}))

vi.mock('../bookAI', () => ({
  resolveAIProviderConfig: vi.fn(() => ({ endpoint: '', modelId: 'test', apiKey: '', providerName: 'boxplayer-cloud' })),
  generateAIText: vi.fn(async () => '{"type":"unknown","title":"","confidence":0,"allowOverwrite":false,"reason":"mock"}')
}))

const storage = new Map<string, string>()
vi.stubGlobal('localStorage', {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => { storage.set(key, String(value)) },
  removeItem: (key: string) => { storage.delete(key) },
  clear: () => { storage.clear() },
  key: (index: number) => Array.from(storage.keys())[index] ?? null,
  get length() { return storage.size }
})

describe('mediaAIScrape', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('parses JSON decisions wrapped in markdown fences', async () => {
    const { parseMediaAIScrapeDecision } = await import('../mediaAIScrape')

    const decision = parseMediaAIScrapeDecision('```json\n{"type":"tv","title":"The Bear","year":2022,"season":2,"episode":3,"tmdbId":136315,"confidence":0.91,"allowOverwrite":true,"reason":"folder context"}\n```')

    expect(decision).toMatchObject({
      type: 'tv',
      title: 'The Bear',
      year: 2022,
      season: 2,
      episode: 3,
      tmdbId: 136315,
      confidence: 0.91,
      allowOverwrite: true
    })
  })

  it('rejects unknown and low confidence decisions', async () => {
    const { normalizeMediaAIScrapeDecision } = await import('../mediaAIScrape')

    expect(normalizeMediaAIScrapeDecision({ type: 'unknown', title: 'x', confidence: 0.9 }).usable).toBe(false)
    expect(normalizeMediaAIScrapeDecision({ type: 'movie', title: 'x', confidence: 0.42 }).usable).toBe(false)
  })

  it('gates automatic AI scraping to Pro users without opening pricing', async () => {
    const { canUseMediaAIScrape } = await import('../mediaAIScrape')

    expect(canUseMediaAIScrape({ manual: false }).allowed).toBe(false)
    expect(localStorage.getItem('boxplayer_show_pricing')).toBeNull()

    localStorage.setItem('app_user_pro', '1')
    expect(canUseMediaAIScrape({ manual: false }).allowed).toBe(true)
  })

  it('opens pricing for manual non-Pro requests', async () => {
    const { canUseMediaAIScrape } = await import('../mediaAIScrape')

    const result = canUseMediaAIScrape({ manual: true })

    expect(result.allowed).toBe(false)
    expect(result.message).toContain('AI 影视刮削')
    expect(localStorage.getItem('boxplayer_show_pricing')).toBe('1')
  })

  it('maps an AI movie decision through TMDB and preserves drive files', async () => {
    const { applyAIScrapeResult } = await import('../mediaAIScrape')
    const file = driveFile()
    const movie: MovieItem = {
      id: 42,
      title: 'Arrival',
      original_title: 'Arrival',
      original_language: 'en',
      overview: 'A linguist meets visitors.',
      release_date: '2016-11-11',
      vote_average: 8.1,
      genres: [{ id: 1, name: '科幻' }],
      credits: {},
      production_countries: [{ iso31661: 'US', name: '美国' }]
    }

    const item = await applyAIScrapeResult(
      { file, folderName: 'Movies', folderId: 'folder-1' },
      { ok: true, decision: { type: 'movie', title: 'Arrival', year: 2016, tmdbId: 42, confidence: 0.93, allowOverwrite: true, reason: 'exact' }, provider: 'test' },
      { tmdb: { searchMovie: vi.fn(async () => movie) } as any }
    )

    expect(item).toMatchObject({
      type: 'movie',
      name: 'Arrival',
      tmdbId: 42,
      metadataSource: 'ai-tmdb'
    })
    expect(item?.driveFiles).toEqual([file])
    expect(item?.aiScrape?.confidence).toBe(0.93)
  })

  it('maps an AI TV decision to the requested episode', async () => {
    const { applyAIScrapeResult } = await import('../mediaAIScrape')
    const file = driveFile({ id: 'ep-file', name: 'messy.name.203.mkv' })
    const tv: MediaLibraryTvSeriesItem = {
      id: 100,
      tv: {
        id: 100,
        name: 'Some Show',
        overview: 'show',
        first_air_date: '2020-01-01',
        vote_average: 7.5,
        genres: [{ id: 2, name: '剧情' }]
      },
      current_season: {
        id: 200,
        season_number: 2,
        name: 'Season 2',
        episode_count: 10,
        episodes: [
          { id: 203, name: 'Episode 3', season_number: 2, episode_number: 3 }
        ]
      }
    }

    const item = await applyAIScrapeResult(
      { file, folderName: 'Some Show', folderId: 'folder-2' },
      { ok: true, decision: { type: 'tv', title: 'Some Show', season: 2, episode: 3, confidence: 0.88, allowOverwrite: true, reason: 'episode naming' }, provider: 'test' },
      { tmdb: { searchTV: vi.fn(async () => tv) } as any }
    )

    expect(item?.type).toBe('tv')
    expect(item?.seasons?.[0].episodes?.[0]).toMatchObject({
      seasonNumber: 2,
      episodeNumber: 3,
      driveFiles: [file]
    })
  })
})

function driveFile(overrides: Partial<DriveFileItem> = {}): DriveFileItem {
  return {
    id: 'file-1',
    name: 'Arrival.2016.mkv',
    path: '/Movies/Arrival.2016.mkv',
    userId: 'u1',
    driveId: 'd1',
    driveServerId: 'aliyun',
    fileSize: 1024,
    contentHash: 'hash',
    ...overrides
  }
}
