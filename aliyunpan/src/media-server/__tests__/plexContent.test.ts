import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  getMediaServerDownloadInfo,
  getMediaServerFilteredPagedItems,
  getMediaServerHomeLatest,
  getMediaServerHomeShell,
  getMediaServerItemDetail,
  getMediaServerPlaybackInfo,
  getMediaServerSearch,
  updateMediaServerFavoriteState,
  updateMediaServerPlayedState
} from '../contentGateway'
import type { MediaServerConfig } from '../../types/mediaServer'

const plexConfig: MediaServerConfig = {
  id: 'plex-1',
  type: 'plex',
  name: 'Plex',
  baseUrl: 'http://127.0.0.1:32400',
  accessToken: 'plex-token',
  userId: 'plex-user',
  deviceId: 'plex-device',
  createdAt: 1,
  updatedAt: 1
}

const plexMovie = {
  ratingKey: 'movie-1',
  key: '/library/metadata/movie-1',
  type: 'movie',
  title: 'Test Movie',
  summary: 'Movie summary',
  year: 2025,
  thumb: '/library/metadata/movie-1/thumb/1',
  art: '/library/metadata/movie-1/art/1',
  duration: 7_200_000,
  viewOffset: 1_800_000,
  viewCount: 1,
  userRating: 10,
  Genre: [{ tag: '科幻' }],
  Studio: [{ tag: 'Studio A' }],
  Role: [{ id: 'person-1', tag: 'Actor A', role: '主演', thumb: '/library/people/person-1/thumb' }],
  Guid: [{ id: 'imdb://tt1234567' }],
  Media: [{
    id: 'media-1',
    duration: 7_200_000,
    bitrate: 8_000_000,
    width: 1920,
    height: 1080,
    Part: [{
      id: 'part-1',
      key: '/library/parts/part-1/file.mkv',
      file: '/movies/Test Movie.mkv',
      size: 123456789,
      Stream: [
        { id: 'stream-1', streamType: 1, index: 0, codec: 'h264', width: 1920, height: 1080, bitrate: 8_000_000 },
        { id: 'stream-2', streamType: 2, index: 1, codec: 'aac', language: 'chi', channels: 2, default: 1 },
        { id: 'stream-3', streamType: 3, index: 2, codec: 'srt', language: 'chi', default: 1 }
      ]
    }]
  }]
}

const plexShow = {
  ratingKey: 'show-1',
  key: '/library/metadata/show-1',
  type: 'show',
  title: 'Test Show',
  summary: 'Show summary',
  year: 2024,
  thumb: '/library/metadata/show-1/thumb/1',
  art: '/library/metadata/show-1/art/1',
  childCount: 1
}

const mediaContainer = (payload: Record<string, any>) => ({
  MediaContainer: payload
})

const response = (payload: any) => ({
  ok: true,
  json: async () => payload,
  arrayBuffer: async () => new ArrayBuffer(0)
})

const installPlexFetchMock = () => {
  const fetchMock = vi.fn(async (rawUrl: string) => {
    const url = String(rawUrl)
    if (url.includes('/library/sections/1/genre')) {
      return response(mediaContainer({
        Directory: [{ key: '10', title: '科幻' }]
      }))
    }
    if (url.includes('/library/sections/2/genre')) {
      return response(mediaContainer({
        Directory: []
      }))
    }
    if (url.includes('/library/sections') && !url.includes('/all')) {
      return response(mediaContainer({
        Directory: [
          { key: '1', title: 'Movies', type: 'movie', childCount: 2 },
          { key: '2', title: 'Shows', type: 'show', childCount: 1 }
        ]
      }))
    }
    if (url.includes('/library/sections/1/all')) {
      return response(mediaContainer({
        totalSize: 2,
        Metadata: [plexMovie]
      }))
    }
    if (url.includes('/library/sections/2/all')) {
      return response(mediaContainer({
        totalSize: 1,
        Metadata: [plexShow]
      }))
    }
    if (url.includes('/library/metadata/movie-1')) {
      return response(mediaContainer({
        Metadata: [plexMovie]
      }))
    }
    if (url.includes('/hubs/search')) {
      return response(mediaContainer({
        Hub: [
          { title: 'Movies', Metadata: [plexMovie] },
          { title: 'Shows', Metadata: [plexShow] }
        ]
      }))
    }
    if (url.includes('/:')) {
      return {
        ok: true,
        json: async () => ({}),
        arrayBuffer: async () => new ArrayBuffer(0)
      }
    }
    throw new Error(`Unexpected Plex URL: ${url}`)
  })
  vi.stubGlobal('fetch', fetchMock)
  return fetchMock
}

describe('Plex content adapter', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('loads Plex library sections and maps them to home shell libraries', async () => {
    const fetchMock = installPlexFetchMock()

    const shell = await getMediaServerHomeShell(plexConfig)

    expect(shell.libraries.map((library) => library.title)).toEqual(['Movies', 'Shows'])
    expect(shell.libraries[0].collectionType).toBe('movies')
    expect(shell.statistics.movieCount).toBe(2)
    expect(fetchMock.mock.calls[0][0]).toContain('/library/sections')
  })

  it('loads recently added across Plex sections without using Jellyfin user item paths', async () => {
    const fetchMock = installPlexFetchMock()

    const latest = await getMediaServerHomeLatest(plexConfig)

    expect(latest.items.map((item) => item.title)).toContain('Test Movie')
    expect(latest.items.map((item) => item.title)).toContain('Test Show')
    expect(fetchMock.mock.calls.some(([url]) => String(url).includes('/Users/'))).toBe(false)
  })

  it('maps Plex metadata details, images, people, media streams and external ids', async () => {
    installPlexFetchMock()

    const detail = await getMediaServerItemDetail(plexConfig, 'movie-1')

    expect(detail.title).toBe('Test Movie')
    expect(detail.poster).toContain('/photo/:/transcode')
    expect(detail.poster).toContain('X-Plex-Token=plex-token')
    expect(detail.genres).toEqual(['科幻'])
    expect(detail.studios).toEqual(['Studio A'])
    expect(detail.people[0]).toEqual(expect.objectContaining({ id: 'person-1', name: 'Actor A', role: '主演' }))
    expect(detail.externalLinks[0].url).toContain('tt1234567')
    expect(detail.sourceOptions[0].id).toBe('part-1')
    expect(detail.mediaInfoCards.map((card) => card.kind)).toEqual(['video', 'audio', 'subtitle'])
  })

  it('uses Plex part keys for playback and download URLs', async () => {
    installPlexFetchMock()

    const playback = await getMediaServerPlaybackInfo(plexConfig, 'movie-1', 'part-1')
    const download = await getMediaServerDownloadInfo(plexConfig, 'movie-1', 'part-1')

    expect(playback.url).toContain('/library/parts/part-1/file.mkv')
    expect(playback.url).toContain('X-Plex-Token=plex-token')
    expect(playback.playCursorSeconds).toBe(1800)
    expect(download.fileName).toBe('Test Movie.mkv')
    expect(download.url).toContain('/library/parts/part-1/file.mkv')
  })

  it('routes search and watched/favorite actions through Plex endpoints', async () => {
    const fetchMock = installPlexFetchMock()

    const search = await getMediaServerSearch(plexConfig, 'test')
    await updateMediaServerPlayedState(plexConfig, 'movie-1', false)
    await updateMediaServerFavoriteState(plexConfig, 'movie-1', false)

    expect(search.items.map((item) => item.title)).toEqual(['Test Movie', 'Test Show'])
    expect(fetchMock.mock.calls.some(([url]) => String(url).includes('/hubs/search'))).toBe(true)
    expect(fetchMock.mock.calls.some(([url]) => String(url).includes('/:\/scrobble'))).toBe(true)
    expect(fetchMock.mock.calls.some(([url]) => String(url).includes('/:\/rate'))).toBe(true)
  })

  it('resolves Plex genre names to section filter keys before loading filtered pages', async () => {
    const fetchMock = installPlexFetchMock()

    await getMediaServerFilteredPagedItems(plexConfig, 0, { genre: '科幻' })

    expect(fetchMock.mock.calls.some(([url]) => String(url).includes('/library/sections/1/genre'))).toBe(true)
    expect(fetchMock.mock.calls.some(([url]) => String(url).includes('/library/sections/1/all') && String(url).includes('genre=10'))).toBe(true)
  })
})
