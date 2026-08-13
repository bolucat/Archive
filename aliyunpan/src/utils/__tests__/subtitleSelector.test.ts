import { describe, expect, it } from 'vitest'
import { dedupeSubtitleSelectors, selectSingleSubtitleCandidates } from '../subtitleSelector'

describe('dedupeSubtitleSelectors', () => {
  it('keeps only the latest copy when a downloaded subtitle also appears in the cloud directory', () => {
    const directory = { file_id: 'cloud-file', name: '天龍八部之虛竹傳奇20.srt', html: '天龍八部之虛竹傳奇20.srt' }
    const downloaded = { name: '天龍八部之虛竹傳奇20.srt', html: '搜索: 天龍八部之虛竹傳奇20.srt', data: 'subtitle', default: true }

    expect(dedupeSubtitleSelectors([directory, downloaded])).toEqual([downloaded])
  })

  it('does not merge subtitles with different labels', () => {
    const first = { name: 'episode.zh-CN.srt' }
    const second = { name: 'episode.zh-TW.srt' }

    expect(dedupeSubtitleSelectors([first, second])).toEqual([first, second])
  })

  it('keeps every playable subtitle for the single-subtitle menu', () => {
    const subtitles = [
      { name: 'zh-Hans', url: 'https://example.com/zh.vtt' },
      { name: 'zh-Hant', url: 'https://example.com/zh-hant.vtt' },
      { name: 'English', url: 'https://example.com/en.vtt' },
      { name: 'Unavailable' }
    ]

    expect(selectSingleSubtitleCandidates(subtitles).map((item) => item.name)).toEqual(['zh-Hans', 'zh-Hant', 'English'])
  })
})
