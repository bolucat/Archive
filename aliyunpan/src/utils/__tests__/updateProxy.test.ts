import { describe, expect, it } from 'vitest'
import { buildUpdateProxyUrl } from '../updateProxy'

describe('buildUpdateProxyUrl', () => {
  it('prefixes a complete GitHub asset URL using gh-proxy format', () => {
    expect(buildUpdateProxyUrl('https://gh-proxy.com/', 'https://github.com/gaozhangmin/boxplayer/releases/download/v5.0.8/BoxPlayer.exe')).toBe('https://gh-proxy.com/https://github.com/gaozhangmin/boxplayer/releases/download/v5.0.8/BoxPlayer.exe')
  })

  it('does not proxy a non-GitHub URL', () => {
    expect(buildUpdateProxyUrl('https://gh-proxy.com', 'https://example.com/BoxPlayer.exe')).toBe('https://example.com/BoxPlayer.exe')
  })
})
