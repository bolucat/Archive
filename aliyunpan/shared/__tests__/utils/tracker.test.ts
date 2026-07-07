import { describe, expect, it } from 'vitest'
import { reduceTrackerString, fetchBtTrackerFromSource } from '../../utils/tracker'

describe('reduceTrackerString', () => {
  it('returns string as-is when within max length', () => {
    const input = 'udp://a.com:80,http://b.com/ann'
    expect(reduceTrackerString(input)).toBe(input)
  })

  it('truncates string when exceeding max length', () => {
    const long = Array.from({ length: 200 }, (_, i) => `udp://tracker${i}.example.com:6881`).join(',')
    const result = reduceTrackerString(long)
    expect(result.length).toBeLessThan(long.length)
  })

  it('handles empty string', () => {
    expect(reduceTrackerString('')).toBe('')
  })
})

describe('fetchBtTrackerFromSource', () => {
  it('returns an array', async () => {
    const result = await fetchBtTrackerFromSource([], {})
    expect(Array.isArray(result)).toBe(true)
  })
})
