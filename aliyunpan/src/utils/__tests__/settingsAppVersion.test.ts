import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('settings app version', () => {
  it('uses Electron app.getVersion after an installed update', () => {
    const mainSource = readFileSync(resolve(process.cwd(), 'electron/main/core/ipcEvent.ts'), 'utf8')
    const settingSource = readFileSync(resolve(process.cwd(), 'src/setting/SettingUI.vue'), 'utf8')

    expect(mainSource).toContain('appVersion: app.getVersion()')
    expect(settingSource).toContain('data.appVersion')
    expect(settingSource).toContain('installedAppVersion.value = data.appVersion')
  })

  it('checks release metadata directly and only proxies the update asset', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/aliapi/server.ts'), 'utf8')

    expect(source).toContain('static getInstalledVersion()')
    expect(source).toContain('.get(ServerHttp.updateUrl')
    expect(source).toContain('buildUpdateProxyUrl(settingStore.uiUpdateProxyUrl, verData.verUrl)')
    expect(source).toContain('const configVer = this.getInstalledVersion()')
  })

  it('keeps the in-app startup check separate from background downloads', () => {
    const pageMainSource = readFileSync(resolve(process.cwd(), 'src/layout/PageMain.ts'), 'utf8')
    const autoUpdateSource = readFileSync(resolve(process.cwd(), 'electron/main/core/autoUpdate.ts'), 'utf8')

    expect(pageMainSource).toContain('if (useSettingStore().uiLaunchAutoCheckUpdate)')
    expect(autoUpdateSource).not.toContain('checkOnStart')
    expect(autoUpdateSource).toContain("readUpdateProxyPreferences(app.getPath('userData'))")
  })
})
