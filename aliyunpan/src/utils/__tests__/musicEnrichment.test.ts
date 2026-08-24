import { beforeEach, describe, expect, it, vi } from 'vitest'

const musicStore = vi.hoisted(() => ({
  tracks: [] as any[],
  getEnrichmentCandidates: vi.fn(),
  updateTrackEnrichment: vi.fn()
}))
const fetchMusicMetadata = vi.hoisted(() => vi.fn())

vi.mock('../../store/musiclibrary', () => ({ default: () => musicStore }))
vi.mock('../musicMetadata', () => ({ fetchMusicMetadata }))
vi.mock('../debuglog', () => ({ default: { mSaveWarning: vi.fn() } }))

import { enrichMusicLibrary } from '../musicEnrichment'

describe('music library enrichment', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    musicStore.tracks = Array.from({ length: 3 }, (_, index) => ({ id: `track-${index}`, file_name: `Artist - Song ${index}.mp3`, artist: 'Artist', title: `Song ${index}` }))
    musicStore.getEnrichmentCandidates.mockImplementation(async (limit: number, _now: number, excluded: Set<string>) => musicStore.tracks.filter(track => !excluded.has(track.id)).slice(0, limit))
    fetchMusicMetadata.mockResolvedValue({ title: 'filename title', artist: 'Artist', album: '', cover: '', metadataSources: ['filename'], lrc: '', lines: [] })
  })

  it('reports attempted items even when a batch finds no covers, so the next batch can run', async () => {
    vi.useFakeTimers()
    try {
      const result = enrichMusicLibrary(2)
      await vi.runAllTimersAsync()

      await expect(result).resolves.toBe(2)
      expect(fetchMusicMetadata).toHaveBeenCalledTimes(2)
      expect(fetchMusicMetadata).toHaveBeenCalledWith(expect.objectContaining({ includeLyrics: false }))
    } finally {
      vi.useRealTimers()
    }
  })

  it('loads enrichment candidates from the persisted library instead of the visible page', async () => {
    vi.useFakeTimers()
    try {
      const persisted = { id: 'track-500', file_name: 'Artist - Deep Track.mp3', artist: 'Artist', title: 'Deep Track' }
      musicStore.tracks = []
      musicStore.getEnrichmentCandidates.mockResolvedValue([persisted])

      const result = enrichMusicLibrary(1)
      await vi.runAllTimersAsync()

      await expect(result).resolves.toBe(1)
      expect(musicStore.getEnrichmentCandidates).toHaveBeenCalledWith(1, expect.any(Number), expect.any(Set))
      expect(fetchMusicMetadata).toHaveBeenCalledWith(expect.objectContaining({ filename: persisted.file_name }))
    } finally {
      vi.useRealTimers()
    }
  })

  it('persists corrected iTunes text metadata instead of keeping filename guesses', async () => {
    vi.useFakeTimers()
    try {
      musicStore.getEnrichmentCandidates.mockResolvedValue([{ id: 'track-1', file_name: 'Wrong Artist - Wrong Song.mp3', artist: 'Wrong Artist', title: 'Wrong Song', metadata_source: 'filename' }])
      fetchMusicMetadata.mockResolvedValue({ title: 'Correct Song', artist: 'Correct Artist', album: 'Correct Album', cover: 'https://cover.example/correct.jpg', metadataSources: ['filename', 'itunes:metadata'], lrc: '', lines: [] })

      const result = enrichMusicLibrary(1)
      await vi.runAllTimersAsync()
      await result

      expect(musicStore.updateTrackEnrichment).toHaveBeenCalledWith('track-1', expect.objectContaining({
        title: 'Correct Song',
        artist: 'Correct Artist',
        album: 'Correct Album',
        cover_url: 'https://cover.example/correct.jpg',
        metadata_source: 'itunes'
      }))
    } finally {
      vi.useRealTimers()
    }
  })

  it('backs off a transient provider failure instead of retrying every scheduler tick', async () => {
    vi.useFakeTimers()
    try {
      musicStore.tracks = [musicStore.tracks[0]]
      fetchMusicMetadata.mockRejectedValueOnce(new Error('429 Too Many Requests'))

      const first = enrichMusicLibrary(1)
      await vi.runAllTimersAsync()
      await expect(first).resolves.toBe(1)
      const second = enrichMusicLibrary(1)
      await vi.advanceTimersByTimeAsync(0)
      expect(musicStore.getEnrichmentCandidates).toHaveBeenLastCalledWith(1, expect.any(Number), new Set(['track-0']))
      await vi.runAllTimersAsync()
      await expect(second).resolves.toBe(0)

      expect(fetchMusicMetadata).toHaveBeenCalledTimes(1)
    } finally {
      vi.useRealTimers()
    }
  })
})
