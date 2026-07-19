import { describe, expect, it } from 'vitest'
import { buildDirectPlayerInvocation, buildPlayerCommand, formatPlayerArg, parsePlayerParams, redactMpvArgs, shellQuote, shellSplit } from '../mpvPlayerPolicy'
// @ts-expect-error The vendored node-mpv utility has no declaration file.
import mpvUtil from '../../module/node-mpv/lib/util'

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

  it('keeps caller spawn options instead of overwriting them with defaults', () => {
    const options = mpvUtil.mergeDefaultOptions({ spawnOptions: { detached: false, shell: false, windowsVerbatimArguments: false } })
    expect(options.spawnOptions).toMatchObject({ detached: false, shell: false, windowsVerbatimArguments: false })
  })

  it('parses comma-separated custom parameters without deleting meaningful spaces', () => {
    expect(parsePlayerParams('--hwdec=auto, --force-media-title=My Movie, --script-opts="key=a,b"')).toEqual([
      '--hwdec=auto',
      '--force-media-title=My Movie',
      '--script-opts=key=a,b'
    ])
  })

  it('builds macOS open command for mpv.app with args passthrough', () => {
    expect(buildPlayerCommand('darwin', '/Applications/mpv.app')).toBe("open -a '/Applications/mpv.app' --args ")
    expect(buildDirectPlayerInvocation('darwin', '/Applications/mpv.app')).toEqual({
      binary: '/Applications/mpv.app/Contents/MacOS/mpv',
      args: []
    })
  })

  it('quotes shell arguments containing single quotes', () => {
    expect(shellQuote("A'B")).toBe("'A'\\''B'")
  })

  it('redacts tokens before logging MPV arguments', () => {
    expect(redactMpvArgs(['--http-header-fields=Authorization: Bearer secret-token', 'https://example.test/a.mkv?x-oss-signature=secret'])).toEqual([
      '--http-header-fields=Authorization: [REDACTED]',
      '[REDACTED_URL]'
    ])
    expect(redactMpvArgs(['--http-header-fields=authorization: Bearer secret,Cookie: sid=private,Referer: https://example.test'])).toEqual([
      '--http-header-fields=authorization: [REDACTED],Cookie: [REDACTED],Referer: https://example.test'
    ])
  })
})
