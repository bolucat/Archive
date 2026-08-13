import { describe, expect, it } from 'vitest'
import { decodeSubtitleBuffer } from '../subtitleApi'

const toBuffer = (bytes: number[]) => new Uint8Array(bytes).buffer

describe('subtitle encoding', () => {
  it('decodes UTF-16LE subtitles with a BOM', () => {
    const data = toBuffer([0xff, 0xfe, 0x31, 0x00, 0x0d, 0x00, 0x0a, 0x00])
    expect(decodeSubtitleBuffer(data)).toBe('1\r\n')
  })

  it('detects UTF-16LE subtitles without a BOM', () => {
    const data = toBuffer([0x31, 0x00, 0x0d, 0x00, 0x0a, 0x00, 0x32, 0x00, 0x0d, 0x00, 0x0a, 0x00])
    expect(decodeSubtitleBuffer(data)).toBe('1\r\n2\r\n')
  })

  it('keeps UTF-8 subtitles intact', () => {
    const data = new TextEncoder().encode('1\n00:00:01,000 --> 00:00:02,000\nHello').buffer
    expect(decodeSubtitleBuffer(data)).toContain('Hello')
  })
})
