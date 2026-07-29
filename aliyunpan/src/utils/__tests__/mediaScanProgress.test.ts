import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('media scan progress', () => {
  it('passes a 0-to-1 ratio to Arco progress components', () => {
    const navSource = readFileSync(resolve(process.cwd(), 'src/components/MediaLibraryNav.vue'), 'utf8')
    const viewSource = readFileSync(resolve(process.cwd(), 'src/views/MediaLibraryView.vue'), 'utf8')

    expect(navSource).toContain('Math.min(1, Math.max(0, mediaStore.scanProgress / mediaStore.scanTotal))')
    expect(viewSource).toContain('Math.min(1, Math.max(0, mediaStore.scanProgress / mediaStore.scanTotal))')
  })
})
