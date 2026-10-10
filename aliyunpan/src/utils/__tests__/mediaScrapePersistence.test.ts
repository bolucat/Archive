import { beforeAll, describe, expect, it, vi } from 'vitest'
import type { MediaLibraryItem } from '../../types/media'
import { mediaDriveFileKey } from '../mediaSourceMembership'

vi.mock('../tmdb', () => ({ tmdbImageUrl: (path: string) => path }))
let DB: typeof import('../db').default
beforeAll(async () => {
  vi.stubGlobal('self', globalThis)
  DB = (await import('../db')).default
})
const item = (id: string, type: MediaLibraryItem['type'] = 'movie'): MediaLibraryItem => ({ id, type, tmdbId: 123, name: 'Title', parentId: '', genres: [], folderId: 'source', driveFiles: [], addedAt: new Date(0) })

describe('scrape persistence against canonical records', () => {
  it('does not index unmatched, retry-pending, or orphan mappings', async () => {
    const mappings = ['matched', 'unmatched', 'retry', 'missing'].map(mediaId => ({ mediaId, fileId: mediaId }))
    const context = {
      isOpen: () => true,
      imedia_file: { where: () => ({ anyOf: () => ({ toArray: async () => mappings }) }) },
      imedia_item: { bulkGet: async () => [item('matched'), item('unmatched', 'unmatched'), { ...item('retry'), scrapeRetrying: true }, undefined] }
    }
    expect([...await DB.getIndexedMediaFileIds.call(context as any, mappings.map(m => m.fileId))]).toEqual(['matched'])
  })

  it('finds a legacy ID by media type and TMDB identity, not title or cached page', async () => {
    const rows = [item('movie-legacy'), item('tv-legacy', 'tv')]
    const context = {
      isOpen: () => true,
      imedia_item: { where: () => ({ equals: () => ({ filter: (predicate: (row: MediaLibraryItem) => boolean) => ({ first: async () => rows.find(predicate) }) }) }) }
    }
    expect((await DB.getScrapedMediaItem.call(context as any, { ...item('tv_123', 'tv'), name: 'Different translation' }))?.id).toBe('tv-legacy')
  })

  it('merges uncached episodes while allowing intentional complete replacement', async () => {
    const file = (id: string) => ({ id, name: `${id}.mkv`, path: '/', driveServerId: 'quark', userId: 'u', driveId: 'd', fileSize: 1 })
    const season = (episodeNumber: number) => ({ id: 1, name: 'Season', seasonNumber: 1, episodeCount: 2, episodes: [{ id: episodeNumber, name: 'Episode', seasonNumber: 1, episodeNumber, driveFiles: [file(`ep${episodeNumber}`)] }] })
    let stored = { ...item('legacy', 'tv'), seasons: [season(1)] }
    let mappings: any[] = []
    const context = {
      isOpen: () => true,
      transaction: async (...args: any[]) => args.at(-1)(),
      imedia_item: { bulkGet: async () => [stored], bulkPut: async (items: any[]) => { stored = items[0] } },
      imedia_file: { where: () => ({ anyOf: () => ({ delete: async () => {} }) }), bulkPut: async (rows: any[]) => { mappings = rows } }
    }
    await DB.upsertMediaLibraryItems.call(context as any, [{ ...item('legacy', 'tv'), seasons: [season(2)] }])
    expect(stored.seasons[0].episodes.map(e => e.episodeNumber)).toEqual([1, 2])
    expect(mappings.map(m => m.fileId)).toEqual([mediaDriveFileKey(file('ep1')), mediaDriveFileKey(file('ep2'))])
    await DB.upsertMediaLibraryItems.call(context as any, [{ ...item('legacy', 'movie'), tmdbId: 456, metadataSource: 'manual', driveFiles: [file('corrected')] }], false)
    expect(stored.type).toBe('movie')
    expect(stored.tmdbId).toBe(456)
    expect(mappings).toHaveLength(1)
  })
})
