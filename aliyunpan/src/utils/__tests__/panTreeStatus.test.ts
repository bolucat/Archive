import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const panDalSource = readFileSync(new URL('../../pan/pandal.ts', import.meta.url), 'utf8')
const panLeftSource = readFileSync(new URL('../../pan/PanLeft.vue', import.meta.url), 'utf8')

describe('cloud folder loading status', () => {
  it('clears the footer status when the full-tree worker times out', () => {
    const handler = panDalSource.slice(panDalSource.indexOf('static async aReLoadDriveSave'), panDalSource.indexOf('static aReLoadDriveProgress'))
    expect(handler).not.toContain("if (error == 'time') return")
    expect(handler).toContain('finishAllDirLoading')
  })
})

describe('main folder tree affordance', () => {
  it('uses expand arrows without Ant Tree connector lines', () => {
    const mainTree = panLeftSource.slice(panLeftSource.indexOf("key='wangpan'"), panLeftSource.indexOf("key='kuaijie'"))
    expect(mainTree).toContain(":show-line='false'")
    expect(mainTree).toContain('class="ant-tree-switcher-lucide"')
    expect(mainTree).toContain('name="iconArrow-Right2"')
  })
})

describe('Aliyun color-label navigation', () => {
  it('uses a color-specific selection path instead of shortcut navigation', () => {
    const colorTree = panLeftSource.slice(panLeftSource.indexOf("class='colortree'"), panLeftSource.indexOf("class='quickdrop'"))
    expect(colorTree).toContain("@select='handleColorTreeSelect'")
    expect(colorTree).not.toContain('pantreeStore.mTreeSelected(e, true)')
  })
})
