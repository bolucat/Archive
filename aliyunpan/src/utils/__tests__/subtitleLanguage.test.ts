import { describe, expect, it } from 'vitest'
import { formatEmbeddedSubtitleLabel } from '../subtitleLanguage'

describe('formatEmbeddedSubtitleLabel', () => {
  it('cleans provider prefixes and exposes the standardized Chinese subtitle code', () => {
    expect(formatEmbeddedSubtitleLabel('内嵌: [内置字幕]Simplified, Singapore')).toBe('中文（简体，新加坡） · zh-Hans-SG')
  })

  it('keeps accessibility flags and uses a BCP-47 language tag', () => {
    expect(formatEmbeddedSubtitleLabel('Latin America (SDH)')).toBe('拉丁美洲西班牙语 · es-419 · SDH')
  })

  it('uses the active UI language for the readable name without changing the code', () => {
    expect(formatEmbeddedSubtitleLabel('por', 'en-US')).toBe('Portuguese · pt')
  })
})
