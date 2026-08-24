import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

describe('music library enrichment scheduling', () => {
  it('schedules metadata enrichment after a search result page is loaded', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/layout/PageMusicLibrary.vue'), 'utf8')
    const watcher = source.slice(source.indexOf('watch(searchQuery'), source.indexOf('const loadMoreTracks'))

    expect(watcher).toContain('.then((loaded) => {')
    expect(watcher).toContain('if (loaded) scheduleEnrich()')
  })
})
