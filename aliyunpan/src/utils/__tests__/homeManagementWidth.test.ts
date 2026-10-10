import { readFileSync } from 'node:fs'
import { expect, it } from 'vitest'

it('keeps the same dialog and list widths at every home-editor depth', () => {
  const component = readFileSync('src/components/UnifiedHomeManagement.vue', 'utf8')
  expect(component).toContain('.unified-home-management-modal.arco-modal{width:660px!important;max-width:calc(100vw - 32px)!important}')
  expect(component).toContain('.management-scroll{width:100%;min-width:0;box-sizing:border-box;scrollbar-gutter:stable}')
  expect(component).toContain('.management-row{width:100%;box-sizing:border-box}')
  expect(readFileSync('src/views/UnifiedMediaLibraryView.vue', 'utf8')).toContain('modal-class="unified-home-management-modal"')
})
