import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

describe('local media detail light theme', () => {
  const source = readFileSync(new URL('../../components/MediaDetail.vue', import.meta.url), 'utf8')
  const light = source.slice(source.indexOf('/* The cinematic rules above'))

  it('scopes the light palette without replacing dark defaults', () => {
    expect(source).toContain('--scraped-detail-surface: #191919')
    expect(light).toContain("body:not([arco-theme='dark']) #xbybody .media-detail")
    expect(light).toContain('--scraped-detail-surface: #fafafa')
    expect(light).toContain('--scraped-detail-copy: #22252b')
  })

  it('covers the header, artwork transition and lower metadata sections', () => {
    for (const selector of ['.detail-header', '.hero-section::before', '.hero-section::after', '.episode-title', '.cast-name', '.cast-role', '.tag-item', '.detail-media-card.selected', '.detail-media-row strong']) {
      expect(light).toContain(selector)
    }
  })
})
