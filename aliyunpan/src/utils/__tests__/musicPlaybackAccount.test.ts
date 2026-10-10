import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

describe('music playback account routing', () => {
  it('hydrates the track account before resolving its provider and URL', () => {
    const source = readFileSync(new URL('../../layout/PageMusic.vue', import.meta.url), 'utf8')
    const resolver = source.slice(source.indexOf('async function resolveUrl('), source.indexOf('async function loadIdx('))
    const hydration = resolver.indexOf('await UserDAL.GetUserTokenFromDB(track.user_id)')
    expect(hydration).toBeGreaterThan(0)
    expect(resolver.indexOf('UserDAL.GetUserToken(track.user_id)')).toBeGreaterThan(hydration)
    expect(resolver.indexOf('await getRawUrl(')).toBeGreaterThan(hydration)
    expect(resolver).toContain("token.tokenfrom !== 'unknown' ? token.tokenfrom : track.tokenfrom")
  })

  it('does not render a second player in the window footer', () => {
    const source = readFileSync(new URL('../../layout/PageMain.vue', import.meta.url), 'utf8')
    const template = source.slice(source.indexOf('<template>'), source.indexOf('</template>'))
    expect(template).not.toContain("class='footer-music-player'")
  })

  it('keeps the audio host mounted when changing playback views', () => {
    const source = readFileSync(new URL('../../layout/PageMain.vue', import.meta.url), 'utf8')
    expect(source).toContain('v-if="musicPlayerStore.pendingLoad" v-show="musicNowPlayingVisible"')
    expect(source).toContain('side-panel embedded full-page')
    expect(source).toContain("event.key === 'Escape' && musicNowPlayingVisible.value")
    const browser = readFileSync(new URL('../../layout/music/MusicLibraryBrowser.vue', import.meta.url), 'utf8')
    expect(browser).toContain('aria-label="播放队列" @click="player.sendCommand(\'queue\')"')
  })
})
