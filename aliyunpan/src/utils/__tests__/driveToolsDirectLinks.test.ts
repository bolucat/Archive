import { describe, expect, it } from 'vitest'
import { formatDirectLinks } from '../drive-tools/directLinks'

describe('drive-tools direct links', () => {
  it('formats plain url list', () => {
    const text = formatDirectLinks([
      { name: 'a.mp4', fileId: '1', driveId: 'guangya', userId: 'u', url: 'https://example.com/a.mp4', size: 1 },
      { name: 'b.mp4', fileId: '2', driveId: 'guangya', userId: 'u', url: 'https://example.com/b.mp4', size: 2 }
    ], 'url')
    expect(text).toBe('https://example.com/a.mp4\nhttps://example.com/b.mp4')
  })

  it('formats aria2 entries with output name and headers', () => {
    const text = formatDirectLinks([
      { name: 'a.mp4', fileId: '1', driveId: 'guangya', userId: 'u', url: 'https://example.com/a.mp4', size: 1, headers: { Referer: 'https://example.com' } }
    ], 'aria2')
    expect(text).toContain('https://example.com/a.mp4')
    expect(text).toContain('  out=a.mp4')
    expect(text).toContain('  header=Referer: https://example.com')
  })
})
