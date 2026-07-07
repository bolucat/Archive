import { describe, expect, it } from 'vitest'
import { normalizeTrackerText, trackerTextToAriaOption } from './tracker'

describe('normalizeTrackerText', () => {
  it('deduplicates and filters tracker lines', () => {
    const input = 'udp://tracker.a.com:80\nudp://tracker.a.com:80\nhttp://tracker.b.com/announce'
    const result = normalizeTrackerText(input)
    expect(result.split('\n').length).toBe(2)
  })

  it('handles comma-separated input', () => {
    const input = 'udp://a.com:80,http://b.com/ann'
    const result = normalizeTrackerText(input)
    expect(result).toContain('udp://a.com:80')
    expect(result).toContain('http://b.com/ann')
  })

  it('filters out non-tracker lines', () => {
    const input = 'udp://good.com:80\nftp://bad.com\n  \nhttp://ok.com/ann'
    expect(normalizeTrackerText(input)).not.toContain('ftp://')
  })
})

describe('trackerTextToAriaOption', () => {
  it('returns comma-joined bt-tracker option', () => {
    const input = 'udp://a.com:80\nhttp://b.com/ann'
    const { 'bt-tracker': bt } = trackerTextToAriaOption(input)
    expect(bt).toBe('udp://a.com:80,http://b.com/ann')
  })
})
