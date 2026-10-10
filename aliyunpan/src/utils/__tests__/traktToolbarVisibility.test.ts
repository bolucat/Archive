import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const source = readFileSync(new URL('../../views/UnifiedMediaLibraryView.vue', import.meta.url), 'utf8')
const button = source.match(/<button\b[^>]*aria-label="Trakt"[^>]*>/)?.[0]
const condition = button?.match(/v-if="([^"]+)"/)?.[1]

describe('Trakt toolbar entry', () => {
  it.each([false, true])('is home-only when connected=%s', connected => {
    expect(condition).toBeDefined()
    const visible = new Function('section', 'traktStatus', `return (${condition})`)
    expect(visible('home', { connected })).toBe(true)
    for (const section of ['video', 'server', 'collection', 'files', 'music', 'book', 'search']) {
      expect(visible(section, { connected })).toBe(false)
    }
  })
})
