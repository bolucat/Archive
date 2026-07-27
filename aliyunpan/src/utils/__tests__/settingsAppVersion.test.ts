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

  it('uses the same installed version and proxy for the legacy update check', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/aliapi/server.ts'), 'utf8')

    expect(source).toContain('static getInstalledVersion()')
    expect(source).toContain('const updateUrl = settingStore.uiUpdateProxyEnable')
    expect(source).toContain('.get(updateUrl')
    expect(source).toContain('const configVer = this.getInstalledVersion()')
  })
})
