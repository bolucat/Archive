import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

describe('Trakt account modal layout', () => {
  it('keeps a constant width while loading status, connecting and signing out', () => {
    const source = readFileSync('src/components/TraktAccountModal.vue', 'utf8')
    expect(source).toContain(':width="384"')
    expect(source).not.toContain(':width="traktStatus.connected')
    expect(source).toContain('await loadTraktStatus()')
    expect(source).toContain('await refreshAccount()')
  })
})
