import { describe, expect, it } from 'vitest'
import { extractExternalDownloadArg, extractExternalFileArg } from '../protocol'

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

describe('extractExternalFileArg', () => {
  it('finds supported local media and book file arguments', () => {
    expect(extractExternalFileArg(['app', '/tmp/demo.mkv'])).toBe('/tmp/demo.mkv')
    expect(extractExternalFileArg(['app', 'C:\\Books\\demo.cbz'])).toBe('C:\\Books\\demo.cbz')
  })

  it('does not treat flags or unsupported files as system-open files', () => {
    expect(extractExternalFileArg(['app', '--profile', 'demo.zip'])).toBe('')
  })
})
