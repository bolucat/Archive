import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const source = (file: string) => readFileSync(new URL(`../../${file}`, import.meta.url), 'utf8')

describe('shared poster placeholder', () => {
  it('matches the reference horizontal film outline, stroke and orange color', () => {
    const svg = source('assets/media/poster-placeholder-film.svg')
    expect(svg).toContain('viewBox="0 0 120 88"')
    expect(svg).toContain('stroke="#ff8000"')
    expect(svg).toContain('stroke-width="8"')
  })

  it('uses the reference solid background and proportional icon instead of the old square icon', () => {
    const component = source('components/MediaPosterPlaceholder.vue')
    expect(component).toContain('background: #232625 !important')
    expect(component).toContain('width: 34.5% !important')
    expect(component).toContain('aspect-ratio: 120 / 88')
    expect(component).toContain('position: static !important')
    expect(component).toContain('<span v-else-if="kind === \'film\'" class="media-poster-placeholder-icon"')
    expect(component).toContain('v-if="kind === \'resume\'"')
    expect(component).toContain('v-html="filmIcon"')
  })

  it.each(['components/UnifiedMediaRow.vue', 'components/MediaLibraryIntegration.vue', 'components/LocalMediaFileCard.vue', 'components/MediaDetail.vue', 'components/MediaLibrary.vue', 'views/MediaServerWorkspace.vue'])('shares the placeholder in %s', file => {
    const page = source(file)
    expect(page).toContain('import MediaPosterPlaceholder')
    expect(page).toContain('<MediaPosterPlaceholder')
  })

  it('overrides the media server dark-theme gradient for poster placeholders only', () => {
    expect(source('views/MediaServerWorkspace.vue')).toContain("body[arco-theme='dark'] #xbybody .media-server-workspace .media-image-placeholder:has(> .media-poster-placeholder-icon) {\n  background: #232625 !important;")
  })
})
