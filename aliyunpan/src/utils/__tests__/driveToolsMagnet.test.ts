import { describe, expect, it } from 'vitest'
import { extractMagnetLinks, getMagnetBtih, getMagnetDisplayName } from '../drive-tools/magnet'

describe('drive-tools magnet', () => {
  it('extracts and deduplicates magnet links', () => {
    const text = [
      'magnet:?xt=urn:btih:ABCDEF1234567890&dn=Movie%20A',
      'magnet:?xt=urn:btih:abcdef1234567890&dn=Movie%20A),',
      '{"url":"magnet:?xt=urn:btih:1234567890abcdef&dn=Movie%20B"}'
    ].join('\n')

    const magnets = extractMagnetLinks(text)
    expect(magnets).toHaveLength(2)
    expect(magnets[0]).toBe('magnet:?xt=urn:btih:ABCDEF1234567890&dn=Movie%20A')
    expect(magnets[1]).toBe('magnet:?xt=urn:btih:1234567890abcdef&dn=Movie%20B')
  })

  it('reads btih and display name', () => {
    const magnet = 'magnet:?xt=urn:btih:ABCDEF1234567890&dn=Movie%20A'
    expect(getMagnetBtih(magnet)).toBe('abcdef1234567890')
    expect(getMagnetDisplayName(magnet)).toBe('Movie A')
  })
})
