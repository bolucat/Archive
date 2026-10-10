import { describe, it, expect } from 'vitest'
import { detailBackdropUrl } from '../mediaArtwork'
import { readFileSync } from 'node:fs'
import { parse, compileScript, compileTemplate } from 'vue/compiler-sfc'
describe('detail hero artwork and bottom alignment', () => {
  it('upgrades existing cached TMDB thumbnail URLs without rescraping', () => {
    expect(detailBackdropUrl('https://image.tmdb.org/t/p/w500/backdrop.jpg')).toBe('https://image.tmdb.org/t/p/original/backdrop.jpg')
    expect(detailBackdropUrl('https://example.test/api/tmdb/image/w500/backdrop.jpg?token=example')).toBe('https://example.test/api/tmdb/image/original/backdrop.jpg?token=example')
  })
  it('preserves unrelated server and local artwork', () => {
    for (const value of ['https://server.test/image/w500/file.jpg', 'file:///test/backdrop.jpg', 'https://image.tmdb.org/t/p/original/file.jpg', '']) expect(detailBackdropUrl(value)).toBe(value)
  })
  it('loads original artwork for grouped banners without enlarging regular poster requests', () => {
    const source = readFileSync('src/components/UnifiedMediaRow.vue', 'utf8')
    expect(source).toContain('row.grouped ? detailBackdropUrl(card.image) : card.image')
    const { descriptor } = parse(source)
    const script = compileScript(descriptor, { id: 'UnifiedMediaRow' })
    expect(compileTemplate({ source: descriptor.template!.content, filename: 'UnifiedMediaRow.vue', id: 'UnifiedMediaRow', compilerOptions: { bindingMetadata: script.bindings } }).errors).toEqual([])
  })
  it('keeps a compact bottom-aligned copy column next to the action column', () => {
    const source = readFileSync('src/components/MediaDetail.vue', 'utf8')
    expect(source).toContain('<div class="hero-copy">')
    expect(source).toMatch(/\.hero-copy\s*\{[^}]*align-self: end;[^}]*flex-direction: column;/)
    expect(source).not.toContain('grid-template-rows: repeat(4, max-content) minmax(0, 1fr)')
    const { descriptor } = parse(source)
    const script = compileScript(descriptor, { id: 'MediaDetail' })
    expect(compileTemplate({ source: descriptor.template!.content, filename: 'MediaDetail.vue', id: 'MediaDetail', compilerOptions: { bindingMetadata: script.bindings } }).errors).toEqual([])
  })
})
