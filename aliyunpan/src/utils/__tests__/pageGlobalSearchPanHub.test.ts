import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const source = readFileSync(new URL('../../layout/PageGlobalSearch.vue', import.meta.url), 'utf8')

describe('PageGlobalSearch PanHub integration', () => {
  it('uses cancellable per-source search instead of the blocking all-source endpoint', () => {
    expect(source).toContain('searchPanHubSources')
    expect(source).toContain('discoverPanHubSources')
    expect(source).toContain('createPanHubFetch')
    expect(source).toContain("panHubFetch(`${PANHUB_API_BASE}/hot-searches`")
    expect(source).not.toContain('src=all')
    expect(source).not.toContain('phBusy')
    expect(source).toContain('watch(searchMode')
  })

  it('shows incremental results while slower sources are still loading', () => {
    expect(source).toContain('<section v-if="phHasResults" class="ph-results-section">')
  })
})
