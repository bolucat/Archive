import { describe, expect, it } from 'vitest'
import { buildDirectPlayerInvocation, buildPlayerCommand, formatPlayerArg, redactMpvArgs, shellQuote, shellSplit } from '../mpvPlayerPolicy'

describe('mpvPlayerPolicy', () => {
  it('builds shell commands without breaking Linux command strings', () => {
    expect(buildPlayerCommand('linux', 'mpv')).toBe('mpv')
    expect(buildPlayerCommand('linux', 'custom-player --flag')).toBe('custom-player --flag')
    expect(buildPlayerCommand('linux', '/opt/mpv/bin/mpv')).toBe("'/opt/mpv/bin/mpv'")
  })

  it('builds unquoted direct-spawn invocation for Linux mpv control mode', () => {
    expect(buildDirectPlayerInvocation('linux', '/usr/bin/mpv')).toEqual({ binary: '/usr/bin/mpv', args: [] })
    expect(buildDirectPlayerInvocation('linux', 'mpv --profile=boxplayer')).toEqual({ binary: 'mpv', args: ['--profile=boxplayer'] })
    expect(buildDirectPlayerInvocation('linux', "'/opt/mpv player/mpv' --profile=boxplayer")).toEqual({ binary: '/opt/mpv player/mpv', args: ['--profile=boxplayer'] })
  })

  it('splits simple quoted Linux command strings', () => {
    expect(shellSplit("mpv --title='A B' --flag")).toEqual(['mpv', '--title=A B', '--flag'])
  })

  it('does not shell-quote arguments for direct spawn', () => {
    expect(formatPlayerArg('linux', 'https://example.test/a b.mp4', true)).toBe('https://example.test/a b.mp4')
    expect(formatPlayerArg('linux', 'https://example.test/a b.mp4', false)).toBe("'https://example.test/a b.mp4'")
  })

  it('builds macOS open command for mpv.app with args passthrough', () => {
    expect(buildPlayerCommand('darwin', '/Applications/mpv.app')).toBe("open -a '/Applications/mpv.app' --args ")
  })

  it('quotes shell arguments containing single quotes', () => {
    expect(shellQuote("A'B")).toBe("'A'\\''B'")
  })

  it('redacts tokens before logging MPV arguments', () => {
    expect(redactMpvArgs(['--http-header-fields=Authorization: Bearer secret-token', 'https://example.test/a.mkv?x-oss-signature=secret'])).toEqual([
      '--http-header-fields=Authorization: [REDACTED]',
      '[REDACTED_URL]'
    ])
  })
})
