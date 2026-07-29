import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('hidden top tabs setting', () => {
  it('uses the checkbox boolean model update instead of reading a DOM event target', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/setting/SettingUI.vue'), 'utf8')
    const pageMainSource = readFileSync(resolve(process.cwd(), 'src/layout/PageMain.vue'), 'utf8')

    expect(source).toContain("@update:model-value='(hidden: boolean) => toggleTopTab(tab.key, hidden)'")
    expect(source).toContain("{ key: 'pan', labelKey: 'nav.pan' }")
    expect(source).not.toContain('event.target.checked')
    expect(pageMainSource).toContain('#xbyhead2 .arco-menu-overflow-wrap')
    expect(pageMainSource).toContain('-webkit-app-region: drag')
    expect(pageMainSource).toContain('#xbyhead2 .arco-menu-horizontal .arco-menu-item *')
    expect(pageMainSource).toContain('-webkit-app-region: no-drag')
    expect(pageMainSource).not.toContain("<div class='title'>BoxPlayer</div>")
  })

  it('shows a correctly directed shared sidebar toggle for every tab with a sidebar', () => {
    const pageMainSource = readFileSync(resolve(process.cwd(), 'src/layout/PageMain.vue'), 'utf8')
    expect(pageMainSource).toContain("const sidebarTabs = new Set(['pan', 'down', 'share', 'rss', 'media', 'media-server', 'music', 'book', 'ai-workspace', 'setting'])")
    expect(pageMainSource).toContain('v-show="hasActiveSidebar"')
    expect(pageMainSource).toContain('<PanelLeftClose v-if=\'activeSidebarVisible\'')
    expect(pageMainSource).toContain('<PanelLeftOpen v-else')
    expect(pageMainSource).not.toContain('name="iconmenuon"')
    expect(pageMainSource).not.toContain('name="iconmenuoff"')
    expect(readFileSync(resolve(process.cwd(), 'src/layout/AISearchAgent.vue'), 'utf8')).toContain("'without-task-rail': !props.sidebarVisible")

    for (const path of ['src/down/index.vue', 'src/share/index.vue', 'src/rss/index.vue', 'src/setting/index.vue', 'src/layout/PageMusicLibrary.vue', 'src/layout/PageBookLibrary.vue']) {
      expect(readFileSync(resolve(process.cwd(), path), 'utf8')).toContain('sidebarVisible')
    }
  })
})
