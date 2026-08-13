import { describe, expect, it } from 'vitest'
import { buildMpvLoadOptions } from '../../mpv/embeddedMpvLoadOptions'

describe('embedded MPV load options', () => {
  it('passes authenticated media headers to loadfile', () => {
    expect(buildMpvLoadOptions({
      headers: {
        Authorization: 'Bearer token-115',
        'User-Agent': 'aDrive/4.12.0'
      },
      startPosition: 108
    })).toBe('user-agent=aDrive/4.12.0,http-header-fields=Authorization: Bearer token-115')
  })

  it('escapes commas inside an Emby authorization header', () => {
    expect(buildMpvLoadOptions({
      headers: {
        'X-Emby-Authorization': 'MediaBrowser Token="token", UserId="user", Client="BoxPlayer"',
        'X-Emby-Token': 'token'
      }
    })).toBe('http-header-fields=X-Emby-Authorization: MediaBrowser Token="token"\\, UserId="user"\\, Client="BoxPlayer",X-Emby-Token: token')
  })

  it('does not emit empty options', () => {
    expect(buildMpvLoadOptions({})).toBe('')
  })
})
