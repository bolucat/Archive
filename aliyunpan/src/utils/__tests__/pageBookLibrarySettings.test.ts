import { describe, expect, it } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'

const read = (file: string) => fs.readFileSync(path.join(process.cwd(), file), 'utf8')

describe('PageBookLibrary Koodo settings', () => {
  it('wires the Koodo settings modal into the book homepage', () => {
    const source = read('src/layout/PageBookLibrary.vue')

    expect(source).toContain('BOOK_MANAGER_SETTING_TABS')
    expect(source).toContain('showManagerSettings')
    expect(source).toContain('manager-settings-modal')
    expect(source).toContain('activeSettingTab')
  })

  it('excludes account and sync settings from the homepage modal', () => {
    const source = read('src/layout/PageBookLibrary.vue')

    expect(source).not.toContain("key: 'account'")
    expect(source).not.toContain("key: 'sync'")
    expect(source).not.toContain('同步和备份')
    expect(source).not.toContain('账户</')
  })

  it('applies appearance preferences to the manager shell', () => {
    const source = read('src/layout/PageBookLibrary.vue')

    expect(source).toContain('managerAppearanceClass')
    expect(source).toContain('managerAppearanceStyle')
    expect(source).toContain('isShowShelfBookCount')
    expect(source).toContain('isDisableCrop')
    expect(source).toContain('isHideShelfBook')
  })
})
