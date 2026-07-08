import { describe, expect, it } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'

const source = fs.readFileSync(path.join(process.cwd(), 'src/layout/PageBookLibrary.vue'), 'utf8')
const pageMainSource = fs.readFileSync(path.join(process.cwd(), 'src/layout/PageMain.vue'), 'utf8')

function cssRuleFrom(cssSource: string, selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const matches = [...cssSource.matchAll(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`, 'g'))]
  return matches.at(-1)?.[1] ?? ''
}

function cssRule(selector: string): string {
  return cssRuleFrom(source, selector)
}

describe('PageBookLibrary tab containment', () => {
  it('keeps Koodo manager chrome inside the app tab pane instead of the viewport', () => {
    expect(cssRule('.book-library')).toContain('position: relative')
    expect(cssRule('.book-sidebar')).not.toContain('position:fixed')
    expect(source).not.toContain('position: fixed')
    expect(cssRule('.book-detail')).not.toContain('position:fixed')
  })

  it('keeps the book list scrollable inside the manager tab', () => {
    const mainRule = cssRule('.book-main')
    const contentRule = cssRule('.book-content')

    expect(mainRule).toContain('flex: 1')
    expect(mainRule).toContain('min-height: 0')
    expect(mainRule).toContain('overflow: hidden')
    expect(mainRule).not.toContain('height: calc(100% - 36px)')
    expect(contentRule).toContain('flex: 1')
    expect(contentRule).toContain('min-height: 0')
    expect(contentRule).toContain('overflow: auto')
  })

  it('keeps global immersive chrome from disabling book list scrolling', () => {
    const globalMainRule = cssRuleFrom(pageMainSource, '#xbybody .book-main')
    const globalContentRule = cssRuleFrom(pageMainSource, '#xbybody .book-content')

    expect(globalMainRule).toContain('display: flex')
    expect(globalMainRule).toContain('flex-direction: column')
    expect(globalMainRule).toContain('overflow: hidden')
    expect(globalContentRule).toContain('flex: 1')
    expect(globalContentRule).toContain('min-height: 0')
    expect(globalContentRule).toContain('overflow-y: auto')
    expect(globalContentRule).toContain('overflow-x: hidden')
  })
})
