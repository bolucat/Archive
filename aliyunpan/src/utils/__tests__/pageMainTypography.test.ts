import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const source = readFileSync(resolve(process.cwd(), 'src/layout/PageMain.vue'), 'utf8')

describe('PageMain typography contract', () => {
  it('defines the shared typography scale for every top-level tab', () => {
    expect(source).toContain('--app-type-caption: 12px')
    expect(source).toContain('--app-type-control: 13px')
    expect(source).toContain('--app-type-body: 14px')
    expect(source).toContain('--app-type-nav: 14px')
    expect(source).toContain('--app-type-section: 17px')
    expect(source).toContain('--app-type-title: 22px')
  })

  it('normalizes shared controls, content rows, navigation and helper text', () => {
    expect(source).toContain('#xbybody .arco-btn,')
    expect(source).toContain('#xbybody .arco-tabs-tab-title,')
    expect(source).toContain('font-size: var(--app-type-control) !important')
    expect(source).toContain('font-size: var(--app-type-body);')
    expect(source).toContain('font-size: var(--app-type-caption) !important')
  })

  it('keeps teleported modals, drawers and dropdowns on the same scale', () => {
    expect(source).toContain('body > .arco-modal-container')
    expect(source).toContain('body > .arco-drawer-container')
    expect(source).toContain('body > .arco-trigger-popup')
  })

  it('normalizes every top-level sidebar navigation label', () => {
    expect(source).toContain('#xbybody .treeleft .arco-tree-node-title,')
    expect(source).toContain('#xbybody .xbyleftmenu .arco-menu-title,')
    expect(source).toContain('#xbybody .book-sidebar .book-nav-item > span,')
    expect(source).toContain('#xbybody .media-library-nav .nav-item > span:first-of-type,')
    expect(source).toContain('#xbybody .media-server-nav .nav-item .server-name,')
    expect(source).toContain('#xbybody .ai-task-rail .ai-new-task,')
    expect(source).toContain('#xbybody .ai-task-rail .ai-rail-action,')
    expect(source).toContain('#xbybody .ai-task-rail .ai-history-item')
    expect(source).toContain('font-size: var(--app-type-nav) !important')
  })
})
