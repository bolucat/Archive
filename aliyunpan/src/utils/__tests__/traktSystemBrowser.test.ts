import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

describe('Trakt system-browser login wiring', () => {
  it('opens the fixed authorization URL externally, without an embedded login window', () => {
    const main = readFileSync('electron/main/trakt.ts', 'utf8')
    expect(main).toContain('shell.openExternal(url.href)')
    expect(main).not.toContain('BrowserWindow')
    expect(main).toContain("new URL('https://auth.trakt.tv/oauth/authorize')")
    expect(main).toContain("code_challenge_method: 'S256'")
    expect(main).toContain("callback.searchParams.get('state') !== state")
    expect(main).toContain('pendingTraktCallback = undefined')
  })
  it('registers and routes the callback only to the main process', () => {
    const launch = readFileSync('electron/main/launch.ts', 'utf8')
    expect(launch).toContain("'boxplayer-traktoauth'")
    expect(launch).toContain('if (handleTraktCallback(url))')
    expect(JSON.parse(readFileSync('electron-builder.json', 'utf8')).protocols[0].schemes).toContain('boxplayer-traktoauth')
    expect(readFileSync('electron/main/core/protocol.ts', 'utf8')).toContain("if (url.startsWith('boxplayer-traktoauth:')) return")
  })
})
