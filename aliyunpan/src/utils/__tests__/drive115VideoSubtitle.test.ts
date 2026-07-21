import { afterEach, describe, expect, it, vi } from 'vitest'
import { mapDrive115SubtitleItems } from '../../cloud115/subtitle'

vi.mock('../../user/userdal', () => ({
  default: {
    GetUserToken: () => ({ access_token: 'test-token' }),
    GetUserTokenFromDB: vi.fn()
  }
}))
vi.mock('../../utils/message', () => ({ default: { error: vi.fn() } }))
vi.mock('../../cloud115/filecmd', () => ({ apiDrive115FileDetailResult: vi.fn() }))

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('mapDrive115SubtitleItems', () => {
  it('maps 115 subtitle URLs into player subtitle sources', () => {
    expect(mapDrive115SubtitleItems([
      { url: 'https://subtitle.115.com/zh.srt', language: 'chi' },
      { url: { url: 'https://subtitle.115.com/en.vtt' }, title: 'English' }
    ])).toEqual([
      { url: 'https://subtitle.115.com/zh.srt', language: 'chi' },
      { url: 'https://subtitle.115.com/en.vtt', language: 'English' }
    ])
  })

  it('drops entries without a usable subtitle URL', () => {
    expect(mapDrive115SubtitleItems([{ language: 'chi' }, null])).toEqual([])
  })

  it('does not block video playback when the subtitle endpoint does not respond', async () => {
    const { apiDrive115VideoSubtitle } = await import('../../cloud115/video')
    const fetchMock = vi.fn((_url: string, init?: RequestInit) => new Promise<Response>((_resolve, reject) => {
      const signal = init?.signal
      signal?.addEventListener('abort', () => reject(signal.reason), { once: true })
    }))
    vi.stubGlobal('fetch', fetchMock)

    await expect(apiDrive115VideoSubtitle('user-115', 'pick-code', 10)).resolves.toEqual([])
    expect(fetchMock).toHaveBeenCalledWith(expect.stringContaining('/open/video/subtitle?pick_code=pick-code'), expect.objectContaining({ signal: expect.any(AbortSignal) }))
  })
})
