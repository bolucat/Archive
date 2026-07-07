import { describe, expect, it } from 'vitest'
import { parseExternalDownloadPayload } from './protocolPayload'

describe('parseExternalDownloadPayload', () => {
  it('accepts magnet links', () => {
    expect(parseExternalDownloadPayload('magnet:?xt=urn:btih:abc'))
      .toEqual({ source: 'magnet:?xt=urn:btih:abc', sourceType: 'magnet' })
  })

  it('accepts torrent URLs', () => {
    expect(parseExternalDownloadPayload('https://example.com/a.torrent'))
      .toEqual({ source: 'https://example.com/a.torrent', sourceType: 'torrent-url' })
  })

  it('accepts http URL', () => {
    expect(parseExternalDownloadPayload('https://example.com/big.zip'))
      .toEqual({ source: 'https://example.com/big.zip', sourceType: 'url' })
  })

  it('accepts local torrent file path', () => {
    expect(parseExternalDownloadPayload('/tmp/a.torrent'))
      .toEqual({ source: '/tmp/a.torrent', sourceType: 'torrent', filePath: '/tmp/a.torrent' })
  })

  it('rejects unsupported schemes', () => {
    expect(parseExternalDownloadPayload('ftp://example.com/a.zip')).toBeNull()
  })

  it('returns null for empty string', () => {
    expect(parseExternalDownloadPayload('')).toBeNull()
  })
})
