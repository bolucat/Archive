import { describe, expect, it, vi } from 'vitest'
import { MpvPlaybackSession } from '../mpvPlaybackSession'

describe('MPV playback subtitle ownership', () => {
  it('does not add remaining old subtitles after changing the video', async () => {
    const session = new MpvPlaybackSession()
    const first = session.begin()
    let finish!: () => void
    const add = vi.fn(() => new Promise<void>((resolve) => { finish = resolve }))
    const pending = session.addSubtitles([{ url: 'old-1' }, { url: 'old-2' }], first, add)
    const second = session.begin()
    finish()
    await pending
    expect(add).toHaveBeenCalledTimes(1)
    const next = vi.fn(async () => {})
    await session.addSubtitles([{ url: 'new' }], second, next)
    expect(next).toHaveBeenCalledWith({ url: 'new' })
  })

  it('deduplicates simultaneous initial-load and subtitle-watcher requests', async () => {
    const session = new MpvPlaybackSession()
    const generation = session.begin()
    let finish!: () => void
    const add = vi.fn(() => new Promise<void>((resolve) => { finish = resolve }))
    const first = session.addSubtitles([{ url: 'subtitle' }], generation, add)
    await session.addSubtitles([{ url: 'subtitle' }], generation, add)
    finish()
    await first
    expect(add).toHaveBeenCalledTimes(1)
  })
})
