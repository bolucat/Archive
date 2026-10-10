import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

describe('list poster indicator positioning', () => {
  it('anchors absolute indicators to the clipped poster rather than the full row', () => {
    const source = readFileSync(new URL('../../components/MediaLibrary.vue', import.meta.url), 'utf8')
    const posterStyle = source.match(/\n\.list-poster\s*\{([^}]+)\}/)?.[1]
    expect(posterStyle).toMatch(/position:\s*relative/)
    expect(posterStyle).toMatch(/overflow:\s*hidden/)
  })
})
