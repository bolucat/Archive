import { describe, expect, it, vi } from 'vitest'
import type { DriveFileItem, MediaLibraryItem, MediaSeason } from '../../types/media'
import { associateScrapedFiles, mergeScrapedMedia, scrapedMediaId } from '../mediaScrapeMerge'

vi.mock('../tmdb', () => ({ tmdbImageUrl: (path: string) => path }))

const file = (id: string, userId = 'user'): DriveFileItem => ({ id, name: `${id}.mkv`, path: `/${id}.mkv`, userId, driveId: 'drive', driveServerId: 'quark', fileSize: 1 })
const item = (changes: Partial<MediaLibraryItem> = {}): MediaLibraryItem => ({ id: 'legacy', type: 'tv', tmdbId: 123, name: 'Show', parentId: '', folderId: 'old-source', genres: [], driveFiles: [], addedAt: new Date(0), ...changes })
const season = (number: number, episodes: number[]): MediaSeason => ({ id: number, name: `Season ${number}`, seasonNumber: number, episodeCount: 10, episodes: episodes.map(episode => ({ id: episode, name: `Episode ${episode}`, seasonNumber: number, episodeNumber: episode, driveFiles: [file(`s${number}e${episode}`)] })) })

describe('scraped media identity and partial merge', () => {
  it('retains the legacy ID, old episodes, all incoming seasons, and file variants', () => {
    const existing = item({ seasons: [season(1, [1, 2])], watchProgress: 42 })
    const incoming = item({ id: 'tv_123', seasons: [season(1, [2, 3]), season(2, [1]), season(3, [1])] })
    incoming.seasons![0].episodes![0].driveFiles = [file('alternate')]
    const result = mergeScrapedMedia(existing, incoming)
    expect(result.id).toBe('legacy')
    expect(result.watchProgress).toBe(42)
    expect(result.seasons?.map(s => s.seasonNumber)).toEqual([1, 2, 3])
    expect(result.seasons?.[0].episodes?.map(e => e.episodeNumber)).toEqual([1, 2, 3])
    expect(result.seasons?.[0].episodes?.[1].driveFiles.map(f => f.id)).toEqual(['s1e2', 'alternate'])
  })

  it('separates movie and TV TMDB namespaces and refuses cross-identity overwrites', () => {
    expect(scrapedMediaId('movie', 123)).not.toBe(scrapedMediaId('tv', 123))
    expect(() => mergeScrapedMedia(item(), item({ type: 'movie' }))).toThrow('身份冲突')
    expect(() => mergeScrapedMedia(item(), item({ tmdbId: 456 }))).toThrow('身份冲突')
  })

  it('preserves manual metadata while adding scraped files', () => {
    const result = mergeScrapedMedia(item({ metadataSource: 'manual', name: 'Corrected', seasons: [season(1, [1])] }), item({ name: 'Search result', seasons: [season(1, [2])] }))
    expect(result.name).toBe('Corrected')
    expect(result.seasons?.[0].episodes).toHaveLength(2)
  })

  it('retains every collection child and both versions of a child movie', () => {
    const movie = (id: number, files: DriveFileItem[]) => ({ ...item({ id: String(id), tmdbId: id, driveFiles: files }), type: 'movie' as const })
    const result = mergeScrapedMedia(item({ type: 'movie', collectionId: 7, collectionMovies: [movie(1, [file('a')])] }), item({ type: 'movie', collectionId: 7, collectionMovies: [movie(1, [file('b')]), movie(2, [file('c')])] }))
    expect(result.collectionMovies?.map(m => m.tmdbId)).toEqual([1, 2])
    expect(result.collectionMovies?.[0].driveFiles.map(f => f.id)).toEqual(['a', 'b'])
  })

  it('attaches overlapping sources without dropping legacy membership or crossing accounts', () => {
    const existing = item({ driveFiles: [file('same')], seasons: [season(1, [1])] })
    const result = associateScrapedFiles(existing, [file('same'), file('s1e1'), file('other'), file('same', 'other-user')], 'new-source')
    expect(result.driveFiles).toHaveLength(1)
    expect(result.driveFiles[0].sourceFolderIds).toEqual(['old-source', 'new-source'])
    expect(result.seasons?.[0].episodes?.[0].driveFiles[0].sourceFolderIds).toEqual(['old-source', 'new-source'])
    expect(existing.driveFiles[0].sourceFolderIds).toBeUndefined()
  })
})
