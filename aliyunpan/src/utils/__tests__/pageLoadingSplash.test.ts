import { readFileSync } from 'fs'
import { resolve } from 'path'
import { describe, expect, it } from 'vitest'

const root = resolve(__dirname, '../../..')
const readSource = (file: string) => readFileSync(resolve(root, file), 'utf8')

describe('app startup loading splash', () => {
  it('uses the BoxPlayer wordmark splash instead of the legacy spinner', () => {
    const source = readSource('src/layout/PageLoading.vue')

    expect(source).toContain('BoxPlayer')
    expect(source).toContain('boxplayer-splash-loading')
    expect(source).toContain('boxplayer-splash-wordmark')
    expect(source).toContain('boxplayer-splash-word-main')
    expect(source).toContain('boxplayer-splash-word-accent')
    expect(source).toContain('boxplayer-splash-line')
    expect(source).toContain('LOADING')
    expect(source).not.toContain('Radio')
    expect(source).not.toContain('desktop-loading-img')
    expect(source).not.toContain('rotate360')
    expect(source).not.toContain('alt="loading-img"')
  })

  it('only renders the splash for app startup and PageMusic windows', () => {
    const app = readSource('src/App.vue')
    const windowSource = readSource('electron/main/core/window.ts')
    const ipcSource = readSource('electron/main/core/ipcEvent.ts')

    expect(app).toContain("splash === 'app' || splash === 'music'")
    expect(app).toContain('PAGE_LOADING_SPLASH_MIN_MS = 1200')
    expect(app).toContain('if (!splashReady.value) return h(PageLoading)')
    expect(app).toContain("return h('div', { class: 'desktop-loading-empty' })")
    expect(windowSource).toContain("'main', AppWindow.winTheme, true, 'app'")
    expect(ipcSource).toContain("data.page === 'PageMusic' ? 'music' : undefined")
  })
})
