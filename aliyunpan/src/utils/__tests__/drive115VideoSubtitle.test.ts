import { describe, expect, it } from 'vitest'
import { mapDrive115SubtitleItems } from '../../cloud115/subtitle'

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
})
