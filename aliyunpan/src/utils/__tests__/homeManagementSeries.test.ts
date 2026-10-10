import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

describe('home management library entries', () => {
  it('shows one favorites entry and includes the series entry with a working route', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/views/UnifiedMediaLibraryView.vue'), 'utf8')
    const nodes = source.slice(source.indexOf('const localNodes:'), source.indexOf('const homeLayout ='))
    expect(nodes).toContain("'custom-series'")
    expect(nodes).not.toContain('local:favorites')
    expect(source).not.toContain('favorites.children =')
    expect(source).toContain("item.id.startsWith('custom-series:') || item.id.startsWith('catalog:series/')")
    expect(source).toContain('ref<CustomMediaSeries[]>(loadCustomSeries())')
    expect(source).toContain("{ id: 'custom-series', title: t('unified.series'), sourceId: 'local', available: true }")
    expect(source).toContain("if (id === 'custom-series') { showCatalog('series'); return }")
    expect(source).toContain("if (!id) { showCatalog('series'); return }")
    expect(source).not.toContain("if (selectedHomeRowKey.value === 'custom-series') return")
  })
  it('uses the shared fixed poster tracks for catalog and search grids', () => {
    const css = readFileSync(resolve(process.cwd(), 'src/styles/mediaPosters.css'), 'utf8')
    expect(css).toContain('.mode-grid:not(:has(.landscape)) .horizontal-row')
    expect(css).toContain('.category-grid:not(.category-list)')
    expect(css).toContain('repeat(auto-fill, var(--media-poster-width)) !important')
  })
})
