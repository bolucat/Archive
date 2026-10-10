import { describe, expect, it } from 'vitest'
import { mediaWatchProgressPercent } from '../mediaWatchProgress'

describe('continue-watching card progress', () => {
  it('converts locally stored fractions to card percentages', () => {
    expect(mediaWatchProgressPercent({ watchProgress: 0.42 })).toBe(42)
    expect(mediaWatchProgressPercent({ watchProgress: 1 })).toBe(100)
    expect(mediaWatchProgressPercent({ watchProgress: 0.005 })).toBe(0.5)
  })
  it('prefers the saved playback position over a stale progress fraction', () => {
    expect(mediaWatchProgressPercent({ watchProgress: 0.1, lastPlayedPositionSeconds: 300, lastPlayedDurationSeconds: 600 })).toBe(50)
    expect(mediaWatchProgressPercent({ watchProgress: 0.8, lastPlayedPositionSeconds: 0, lastPlayedDurationSeconds: 600 })).toBe(0)
  })
  it('handles empty, invalid and legacy percentage records', () => {
    expect(mediaWatchProgressPercent({})).toBe(0)
    expect(mediaWatchProgressPercent({ watchProgress: NaN })).toBe(0)
    expect(mediaWatchProgressPercent({ watchProgress: -1 })).toBe(0)
    expect(mediaWatchProgressPercent({ watchProgress: 42 })).toBe(42)
    expect(mediaWatchProgressPercent({ lastPlayedPositionSeconds: 700, lastPlayedDurationSeconds: 600 })).toBe(100)
  })
})
