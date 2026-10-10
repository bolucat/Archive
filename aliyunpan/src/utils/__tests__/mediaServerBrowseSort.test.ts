import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { compareMediaServerItems, mediaServerSortOptions, randomSortRank } from '../mediaServerBrowseSort'
import type { MediaServerCardItem } from '../../types/mediaServerContent'

const item = (values: Partial<MediaServerCardItem>): MediaServerCardItem => ({ id: '1', serverId: 'server', provider: 'emby', kind: 'movie', title: 'Movie', ...values })

describe('server category sorting', () => {
  it('keeps reference option order', () => {
    expect(mediaServerSortOptions).toEqual(['premiereDate', 'title', 'createdAt', 'rating', 'criticRating', 'runtimeMinutes', 'year', 'playCount', 'sortName', 'addedAt', 'random', 'videoBitrate', 'airTime', 'studio', 'artist', 'officialRating', 'seriesTitle', 'seriesDatePlayed', 'airOrder'])
  })
  it('distinguishes display name from sort name and respects direction', () => {
    const a = item({ title: 'Z', sortName: 'A' }), b = item({ id: '2', title: 'A', sortName: 'Z' })
    expect(compareMediaServerItems(a, b, 'sortName', 'ascending')).toBeLessThan(0)
    expect(compareMediaServerItems(a, b, 'sortName', 'descending')).toBeGreaterThan(0)
    expect(compareMediaServerItems(a, b, 'title', 'ascending')).toBeGreaterThan(0)
  })
  it('uses actual premiere dates, ratings and episode ordering', () => {
    const a = item({ premiereDate: '2025-01-02', rating: 9, seasonNumber: 1, episodeNumber: 10 })
    const b = item({ id: '2', premiereDate: '2025-01-01', rating: 7, seasonNumber: 2, episodeNumber: 1 })
    expect(compareMediaServerItems(a, b, 'premiereDate', 'ascending')).toBeGreaterThan(0)
    expect(compareMediaServerItems(a, b, 'rating', 'descending')).toBeLessThan(0)
    expect(compareMediaServerItems(a, b, 'airOrder', 'ascending')).toBeLessThan(0)
  })
  it.each(['ascending', 'descending'] as const)('keeps missing dates last in %s order', direction => {
    expect(compareMediaServerItems(item({ premiereDate: '2025-01-01' }), item({ id: '2' }), 'premiereDate', direction)).toBeLessThan(0)
  })
  it('does not reshuffle when more pages arrive', () => {
    expect(randomSortRank('server:1', 123)).toBe(randomSortRank('server:1', 123))
    expect(randomSortRank('server:1', 123)).not.toBe(randomSortRank('server:1', 124))
  })
  it('hides server scope tabs and wires a separate sort menu, without local filtering', () => {
    const source = readFileSync('src/views/UnifiedMediaLibraryView.vue', 'utf8')
    const scopeCondition = source.match(/v-if="([^"]+)" class="source-scope"/)?.[1]
    expect(scopeCondition).toContain('isCategoryPage')
    expect(scopeCondition).toContain('!isServerCategoryPage')
    expect(source).toContain('v-for="sort in mediaServerSortOptions"')
    expect(source).toContain(':server-sort-direction="serverSortDirection"')
    const serverTag = source.match(/<ServerWorkspace[^>]+\/>/)?.[0]
    expect(serverTag).not.toContain('local-only')
    expect(serverTag).not.toContain('browse-sort')
  })
})
