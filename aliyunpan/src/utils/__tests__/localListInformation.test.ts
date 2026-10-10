import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

describe('local library list information', () => {
  const source = readFileSync(new URL('../../components/MediaLibrary.vue', import.meta.url), 'utf8')
  it('has one watched toggle in each local list branch', () => {
    expect(source.match(/<WatchedIndicator :watched="isMediaWatched\(item, mediaStore.watchedItems\)" @toggle="toggleLocalMediaWatched\(item\)"/g)).toHaveLength(2)
  })
  it('keeps unified metadata, synopsis, and toggle in reading order', () => {
    const start = source.indexOf('class="list-meta unified-list-meta"')
    const overview = source.indexOf('class="list-overview"', start)
    const toggle = source.indexOf('<WatchedIndicator :watched=', overview)
    expect(start).toBeGreaterThan(0)
    expect(overview).toBeGreaterThan(start)
    expect(toggle).toBeGreaterThan(overview)
    expect(source.slice(start, overview)).toContain('item.certification')
  })
})
