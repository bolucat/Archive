import { describe, expect, it } from 'vitest'
import { Drive115PlaybackAuthRegistry } from '../../drive115PlaybackAuth'

describe('Drive115PlaybackAuthRegistry', () => {
  it('keeps authentication isolated for playback URLs from different accounts', () => {
    const registry = new Drive115PlaybackAuthRegistry()
    registry.register(['https://cpats01.115.com/account-a/master.m3u8?token=a'], { authorization: 'Bearer a', userAgent: '115-agent' }, 10_000)
    registry.register(['https://cpats01.115.com/account-b/master.m3u8?token=b'], { authorization: 'Bearer b', userAgent: '115-agent' }, 10_000)

    expect(registry.resolve('https://cpats01.115.com/account-a/segment-001.ts', 1)?.authorization).toBe('Bearer a')
    expect(registry.resolve('https://cpats01.115.com/account-b/segment-001.ts', 1)?.authorization).toBe('Bearer b')
  })

  it('expires old authentication contexts', () => {
    const registry = new Drive115PlaybackAuthRegistry()
    const now = Date.now()
    registry.register(['https://cdn.115cdn.net/file/video.mp4'], { authorization: 'Bearer old', userAgent: '115-agent' }, now + 60_000)

    expect(registry.resolve('https://cdn.115cdn.net/file/video.mp4?range=1', now + 60_001)).toBeUndefined()
  })
})
