import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { matchesMediaPerson } from '../mediaPersonFilter'
import type { MediaLibraryItem } from '../../types/media'

const item: MediaLibraryItem = { id: 'tv', parentId: 'root', name: 'Show', type: 'tv', genres: [], driveFiles: [], addedAt: new Date() }
describe('detail person navigation', () => {
  it('matches stable IDs even when the person name is localized, without matching other people', () => {
    const media = { ...item, credits: { cast: [{ id: 12, name: 'Actor' }, { id: 13, name: 'Actor Junior' }] } }
    expect(matchesMediaPerson(media, '演员', 12)).toBe(true)
    expect(matchesMediaPerson(media, 'Actor', 99)).toBe(false)
    expect(matchesMediaPerson(media, 'Act')).toBe(false)
    expect(matchesMediaPerson(media, ' actor ')).toBe(true)
  })
  it('includes season cast, episode crew and collection movie credits', () => {
    const season = { id: 1, seasonNumber: 1, name: 'Season', episodeCount: 1, credits: { cast: [{ id: 12, name: 'Actor' }] } }
    expect(matchesMediaPerson({ ...item, seasons: [season] }, '演员', 12)).toBe(true)
    const episode = { id: 2, seasonNumber: 1, episodeNumber: 1, name: 'Episode', driveFiles: [], crew: [{ id: 14, name: 'Director' }] }
    expect(matchesMediaPerson({ ...item, seasons: [{ ...season, credits: undefined, episodes: [episode] }] }, 'Director', 14)).toBe(true)
    expect(matchesMediaPerson({ ...item, collectionMovies: [{ ...item, type: 'movie', credits: season.credits }] }, 'Actor', 12)).toBe(true)
    expect(matchesMediaPerson(item, 'Actor', 12)).toBe(false)
  })
  it('passes the person ID and queries all media rather than the previous category', () => {
    const detail = readFileSync(resolve(process.cwd(), 'src/components/MediaDetail.vue'), 'utf8')
    const library = readFileSync(resolve(process.cwd(), 'src/components/MediaLibrary.vue'), 'utf8')
    expect(detail).toContain("emit('tagClick', 'cast', cast.name, cast.id)")
    expect(library).toContain("const category = selectedCast.value ? 'all' : props.activeCategory || activeTab.value")
    expect(library).toContain('matchesMediaPerson(item, selectedCast.value, selectedPersonId.value)')
  })
})
