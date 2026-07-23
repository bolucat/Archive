import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const readSource = (path: string) => readFileSync(resolve(process.cwd(), path), 'utf8')

const expectLightBackground = (source: string, selector: string) => {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  expect(source).toMatch(new RegExp(`${escapedSelector}\\s*\\{[^}]*background:\\s*var\\(--color-bg-1\\)`, 's'))
}

describe('light theme page surfaces', () => {
  it('lets top tabs use all available header space before window controls', () => {
    const source = readSource('src/layout/PageMain.vue')
    expect(source).not.toMatch(/<\/a-menu>\s*<div class='flexauto'><\/div>\s*<ShutDown \/>/s)
    expect(source).toMatch(/#xbyhead2 \.arco-menu-horizontal\s*\{[^}]*width:\s*0[^}]*flex:\s*1 1 auto/s)
  })

  it('keeps the transfer task canvas on the light page background', () => {
    expectLightBackground(readSource('src/layout/PageMain.vue'), "body:not([arco-theme='dark']) #xbybody .xbyright > .hidetabs")
  })

  it('keeps the plugin canvas on the light page background', () => {
    const source = readSource('src/rss/index.vue')
    expectLightBackground(source, "body:not([arco-theme='dark']) #xbybody .rss-content-panel > .hidetabs")
    expectLightBackground(source, "body:not([arco-theme='dark']) #xbybody .rss-content-panel .rightbg")
  })

  it('keeps the music content canvas on the light page background', () => {
    expectLightBackground(readSource('src/layout/PageMusicLibrary.vue'), "body:not([arco-theme='dark']) .aml-content-area")
  })

  it('uses readable music labels and explicit theme contrast', () => {
    const source = readSource('src/layout/PageMusicLibrary.vue')
    const rail = readSource('src/layout/music/MusicLibraryRail.vue')
    const scanPanel = readSource('src/components/LibraryScanPanel.vue')
    const playlistPanel = readSource('src/components/radio/PlaylistManagerPanel.vue')
    const podcastPanel = readSource('src/components/radio/PodcastPanel.vue')
    expect(source).toContain('--music-ui-text: #fff')
    expect(source).toMatch(/body:not\(\[arco-theme='dark'\]\) \.aml\s*\{[^}]*--music-ui-text:\s*#111827/s)
    expect(source).toMatch(/\.aml-section-label,[^{]*\.aml-track-title[^}]*color:\s*var\(--music-ui-text\)/s)
    expect(source).toMatch(/\.aml-home-tile-title\s*\{[^}]*font-size:\s*13px/s)
    expect(source).toMatch(/\.aml-home-play,[^{]*\.aml-home-ghost[^}]*font-size:\s*13px/s)
    expect(source).toMatch(/body:not\(\[arco-theme='dark'\]\) \.aml \.plm-panel,[^{]*\.podcast-rm[^}]*color:\s*var\(--music-ui-text\)/s)
    expect(rail).toContain('--music-rail-text: #fff')
    expect(rail).toMatch(/body:not\(\[arco-theme='dark'\]\) \.music-rail\s*\{[^}]*--music-rail-text:\s*#111827/s)
    expect(rail).toMatch(/\.music-rail-tab\s*\{[^}]*color:\s*var\(--music-rail-text\)[^}]*font-size:\s*14px/s)
    expect(scanPanel).toMatch(/#xbybody \.music-rail \.library-scan-label\s*\{[^}]*color:\s*#fff[^}]*font-size:\s*13px/s)
    expect(scanPanel).toMatch(/body:not\(\[arco-theme='dark'\]\) #xbybody \.music-rail \.library-scan-label\s*\{[^}]*color:\s*#111827/s)
    expect(playlistPanel).toContain('--music-panel-text: #fff')
    expect(playlistPanel).toMatch(/:global\(body:not\(\[arco-theme='dark'\]\)\) \.plm-panel\s*\{[^}]*--music-panel-text:\s*#111827/s)
    expect(podcastPanel).toContain('--music-panel-text: #fff')
    expect(podcastPanel).toMatch(/:global\(body:not\(\[arco-theme='dark'\]\)\) \.podcast-panel\s*\{[^}]*--music-panel-text:\s*#111827/s)
    expect(podcastPanel).toMatch(/\.podcast-restore\s*\{[^}]*color:\s*var\(--music-ui-text,\s*#fff\)/s)
    expect(podcastPanel).toMatch(/\.podcast-external-title\s*\{[^}]*color:\s*var\(--music-ui-text,\s*#fff\)/s)
    expect(podcastPanel).toMatch(/\.podcast-external-form input\s*\{[^}]*color:\s*var\(--music-ui-text,\s*#111827\)/s)
    expect(podcastPanel).toMatch(/\.podcast-external-form input::placeholder\s*\{[^}]*color:\s*var\(--music-ui-muted,\s*#374151\)[^}]*opacity:\s*1/s)
    expect(podcastPanel).toMatch(/\.podcast-external-form button\s*\{[^}]*color:\s*var\(--music-ui-text,\s*#fff\)/s)
  })

  it('keeps AI workspace columns on light semantic backgrounds', () => {
    const source = readSource('src/layout/AISearchAgent.vue')
    expectLightBackground(source, "body:not([arco-theme='dark']) #xbybody .ai-task-rail")
    expectLightBackground(source, "body:not([arco-theme='dark']) #xbybody .ai-workspace-main")
    expectLightBackground(source, "body:not([arco-theme='dark']) #xbybody .ai-activity-panel")
  })

  it('keeps settings navigation and content on light semantic surfaces', () => {
    const source = readSource('src/setting/index.vue')
    const main = readSource('src/layout/PageMain.vue')
    expect(source).not.toContain("<small>{{ t('settings.centerSubtitle') }}</small>")
    expect(source).toMatch(/\.settings-side-title\s*\{[^}]*height:\s*auto[^}]*min-height:\s*92px[^}]*overflow:\s*visible/s)
    expectLightBackground(source, "body:not([arco-theme='dark']) #xbybody .settings-shell")
    expectLightBackground(source, "body:not([arco-theme='dark']) #xbybody .settings-sider")
    expectLightBackground(source, "body:not([arco-theme='dark']) #xbybody #SettingObserver")
    expect(main).toMatch(/#xbybody \.rightbg,[^{]*#xbybody \.settings-content,[^{]*\{[^}]*border:\s*1px solid/s)
    expect(source).toMatch(/#xbybody #SettingObserver \.settingcard\s*\{[^}]*border:\s*0\s*!important[^}]*border-radius:\s*0\s*!important[^}]*background:\s*transparent\s*!important[^}]*box-shadow:\s*none\s*!important[^}]*backdrop-filter:\s*none\s*!important/s)
    expect(source).toMatch(/body:not\(\[arco-theme='dark'\]\) #xbybody #SettingObserver \.settingcard,[^{]*\.arco-divider-text,[^{]*\.arco-checkbox-label,[^{]*\.arco-switch-text\s*\{[^}]*color:\s*#111827\s*!important/s)
    expect(source).toMatch(/body:not\(\[arco-theme='dark'\]\) #xbybody #SettingObserver \.settingrow,[^{]*\.settings-app-subtitle,[^{]*\.acc-desc\s*\{[^}]*color:\s*#374151\s*!important/s)
    expect(source).toMatch(/body:not\(\[arco-theme='dark'\]\) #xbybody #SettingObserver \.arco-input,[^{]*\.arco-select-view-value,[^{]*\.arco-input-number-input\s*\{[^}]*color:\s*#111827\s*!important/s)
  })

  it('uses the active Arco dark-theme hook for the settings log panel', () => {
    const source = readSource('src/setting/SettingLog.vue')
    expect(source).not.toContain('html.dark .loglist')
    expect(source).toMatch(/body\[arco-theme='dark'\] #xbybody \.loglist\s*\{[^}]*background:\s*#05070a/s)
    expect(source).toMatch(/body\[arco-theme='dark'\] #xbybody \.loglist \.arco-list-item\s*\{[^}]*color:\s*rgba\(232, 238, 249, 0\.88\)/s)
  })

  it('uses a single outer boundary for transfer, share and plugin navigation', () => {
    const main = readSource('src/layout/PageMain.vue')
    const settings = readSource('src/setting/index.vue')
    const sidebars = [readSource('src/down/index.vue'), readSource('src/share/index.vue'), readSource('src/rss/index.vue'), settings]

    for (const sidebar of sidebars) {
      expect(sidebar).toContain('single-boundary-sidebar')
      expect(sidebar).toContain('single-boundary-sidebar-menu')
    }

    expect(main).toMatch(/#xbybody \.single-boundary-sidebar > \.single-boundary-sidebar-menu\s*\{[^}]*padding:\s*0[^}]*border:\s*0\s*!important[^}]*background:\s*transparent\s*!important[^}]*box-shadow:\s*none\s*!important/s)
    expect(main).toMatch(/#xbybody \.single-boundary-sidebar \.single-boundary-sidebar-menu \.arco-menu-item\s*\{[^}]*border:\s*0\s*!important[^}]*box-shadow:\s*none\s*!important/s)
    expect(main).toMatch(/#xbybody \.single-boundary-sidebar \.single-boundary-sidebar-menu \.arco-menu-selected\s*\{[^}]*border-color:\s*transparent\s*!important[^}]*box-shadow:\s*none\s*!important/s)
    expect(main).toMatch(/#xbybody \.single-boundary-sidebar \.single-boundary-sidebar-menu \.arco-menu-item::before,[^{]*\.arco-menu-item::after\s*\{[^}]*display:\s*none\s*!important/s)
    expect(settings).not.toMatch(/body\[arco-theme='dark'\] \.xbyleftmenu/)
    expect(settings).not.toMatch(/^\.xbyleftmenu/m)
    expect(settings).toMatch(/\.settings-sider \.xbyleftmenu\s*\{[^}]*padding:\s*0[^}]*border:\s*0[^}]*border-radius:\s*0[^}]*background:\s*transparent\s*!important[^}]*box-shadow:\s*none\s*!important/s)
    expect(settings).toMatch(/body\[arco-theme='dark'\] \.settings-sider \.xbyleftmenu\s*\{[^}]*background:\s*transparent\s*!important[^}]*border:\s*0\s*!important[^}]*box-shadow:\s*none\s*!important/s)
  })

  it('uses readable AI workspace labels and explicit theme contrast', () => {
    const agent = readSource('src/layout/AISearchAgent.vue')
    const workspace = readSource('src/layout/PageAIWorkspace.vue')
    expect(agent).toContain('--agent-ui-text: #fff')
    expect(agent).toMatch(/body:not\(\[arco-theme='dark'\]\) #xbybody \.ai-chat\s*\{[^}]*--agent-ui-text:\s*#111827/s)
    expect(agent).toMatch(/\.ai-rail-label[^}]*font-size:\s*13px[^}]*color:\s*var\(--agent-ui-text\)/s)
    expect(agent).toMatch(/\.ai-rail-action,[^{]*\.ai-history-item[^}]*font-size:\s*13px[^}]*color:\s*var\(--agent-ui-text\)/s)
    expect(agent).toMatch(/\.ai-rail-action:disabled\s*\{[^}]*color:\s*var\(--agent-ui-text\)[^}]*opacity:\s*1/s)
    expect(workspace).toMatch(/\.ai-workspace-view-switcher button[^}]*color:\s*#fff[^}]*font-size:\s*13px/s)
    expect(workspace).toMatch(/body:not\(\[arco-theme='dark'\]\) \.ai-workspace-view-switcher button\s*\{[^}]*color:\s*#111827/s)
  })
})
