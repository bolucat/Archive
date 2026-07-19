import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const source = readFileSync(new URL('../../layout/PageGlobalSearch.vue', import.meta.url), 'utf8')

describe('PageGlobalSearch PanHub integration', () => {
  it('uses a cancellable streaming search request', () => {
    expect(source).toContain('createPanHubFetch')
    expect(source).toContain('searchPanHubStream')
    expect(source).toContain("panHubFetch(`${PANHUB_API_BASE}/hot-searches`")
    expect(source).not.toContain('phBusy')
    expect(source).toContain('watch(searchMode')
  })

  it('leaves source selection and time limits to the server', () => {
    expect(source).not.toContain('PanHubSettingsDrawer')
    expect(source).not.toContain('discoverPanHubSources')
    expect(source).not.toContain('pluginTimeoutMs:')
  })

  it('shows incremental results while slower sources are still loading', () => {
    expect(source).toContain('<section v-if="phHasResults" class="ph-results-section">')
  })
})
