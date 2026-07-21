import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const source = readFileSync(new URL('../../layout/PageGlobalSearch.vue', import.meta.url), 'utf8')
const mediaResultsSource = readFileSync(new URL('../../layout/PanHubMediaResults.vue', import.meta.url), 'utf8')
const acquisitionModalSource = readFileSync(new URL('../../components/MediaAcquisitionTargetModal.vue', import.meta.url), 'utf8')

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

  it('shows acquisition progress beside the media copy instead of over the poster', () => {
    expect(mediaResultsSource).toContain("'media-acquisition-panel'")
    expect(mediaResultsSource).toContain('class="media-acquisition-track"')
    expect(mediaResultsSource).toContain('class="media-acquisition-activity"')
    expect(mediaResultsSource).toContain('<LoaderCircle')
    expect(mediaResultsSource).toContain('role="progressbar"')
    expect(mediaResultsSource).not.toContain('media-poster-acquisition-state')
    expect(mediaResultsSource).not.toContain('grid-column: 1 / -1')
    expect(mediaResultsSource).not.toContain("'has-acquisition-state': stateOf(item)")
    expect(mediaResultsSource).toMatch(/media-search-title-row[\s\S]*media-acquisition-panel[\s\S]*<\/div>\s*<p class="media-search-meta">/)
    expect(mediaResultsSource).toContain('flex: 0 0 92px;')
  })

  it('confirms and retries a completed media acquisition with force enabled', () => {
    expect(acquisitionModalSource).toContain('isCompletedDuplicateError(error)')
    expect(acquisitionModalSource).toContain('确定再次创建获取任务吗？')
    expect(acquisitionModalSource).toContain('createMediaAcquisitionRun({ ...runInput, force: true })')
  })
})
