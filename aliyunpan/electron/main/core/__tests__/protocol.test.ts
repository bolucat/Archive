import { describe, expect, it } from 'vitest'
import { extractExternalDownloadArg } from '../protocol'

describe('extractExternalDownloadArg', () => {
  it('finds a magnet argument', () => {
    expect(extractExternalDownloadArg(['app', 'magnet:?xt=urn:btih:abc'])).toBe('magnet:?xt=urn:btih:abc')
  })

  it('finds a torrent file argument', () => {
    expect(extractExternalDownloadArg(['app', '/tmp/a.torrent'])).toBe('/tmp/a.torrent')
  })

  it('finds a torrent URL argument', () => {
    expect(extractExternalDownloadArg(['app', 'https://ex.com/a.torrent'])).toBe('https://ex.com/a.torrent')
  })

  it('returns empty string when nothing matches', () => {
    expect(extractExternalDownloadArg(['app', '--flag', 'https://ex.com/file.zip'])).toBe('')
  })
})
