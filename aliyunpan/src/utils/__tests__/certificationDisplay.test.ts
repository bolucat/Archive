import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const source = (path: string) => readFileSync(new URL('../../' + path, import.meta.url), 'utf8')
describe('certification display', () => {
  it('shows the real certification as an outlined detail badge', () => {
    const detail = source('components/MediaDetail.vue')
    expect(detail).toContain('v-if="activeMediaItem.certification?.trim()"')
    expect(detail).toContain('{{ activeMediaItem.certification }}')
    expect(detail).toMatch(/\.content-certification\s*\{[^}]*border: 1px solid currentColor/)
  })
  it('keeps certification in both local list entry points', () => {
    expect(source('components/UnifiedMediaRow.vue')).toContain('{{ card.certification }}')
    expect(source('components/MediaLibrary.vue')).toContain('{{ item.certification }}')
  })
})
