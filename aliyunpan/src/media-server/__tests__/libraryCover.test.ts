import { describe, expect, it, vi } from 'vitest'
import { libraryCover, withLibraryCoverFallback } from '../libraryCover'
import type { MediaServerCardItem } from '../../types/mediaServerContent'

const library: MediaServerCardItem = { id: 'library', serverId: 'server', provider: 'emby', kind: 'folder', title: 'Movies' }

describe('library covers', () => {
  it('preserves the library primary cover instead of child posters', () => {
    const own = { ...library, images: { primary: 'own', backdrop: 'background' } }
    expect(withLibraryCoverFallback(own, [{ ...library, poster: 'child' }])).toBe(own)
    expect(libraryCover(own)).toBe('own')
  })
  it('preserves other library cover types', () => {
    for (const own of [{ ...library, poster: 'own' }, { ...library, images: { thumb: 'own' } }, { ...library, backdrop: 'own' }]) {
      expect(withLibraryCoverFallback(own, [{ ...library, poster: 'child' }])).toBe(own)
    }
  })
  it('randomly selects only children with posters for a missing cover', () => {
    const random = vi.spyOn(Math, 'random').mockReturnValue(0.99)
    try {
      const result = withLibraryCoverFallback(library, [library, { ...library, poster: 'first' }, { ...library, images: { primary: 'second' } }])
      expect(libraryCover(result)).toBe('second')
      expect(libraryCover(library)).toBe('')
    } finally { random.mockRestore() }
  })
  it('keeps the placeholder when no child has a poster', () => {
    expect(withLibraryCoverFallback(library, [library])).toBe(library)
  })
})
