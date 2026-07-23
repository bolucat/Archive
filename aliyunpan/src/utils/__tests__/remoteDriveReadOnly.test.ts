import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { getProviderCapabilities } from '../../services/agent/providerCapabilities'

describe.each(['webdav', 'alist'])('%s remote drive capabilities', platform => {
  it('is read-only', () => {
    const operations = getProviderCapabilities(platform).operations
    expect(operations['files.list']).toBe(true)
    expect(operations['files.download']).toBe(true)
    expect(Object.entries(operations).filter(([operation]) => !['files.list', 'files.download'].includes(operation)).every(([, supported]) => !supported)).toBe(true)
  })
})

it('maps both virtual account types and drive ids to the read-only manifest', () => {
  const source = readFileSync(new URL('../../aliapi/providerFeatures.ts', import.meta.url), 'utf8')
  expect(source).toContain('isRemoteDriveUser(userId)')
  expect(source).toContain("drive.startsWith('webdav:')")
  expect(source).toContain("return 'webdav'")
})

it('gates file-page mutations with provider capabilities', () => {
  const files = [
    new URL('../../pan/menus/PanTopbtn.vue', import.meta.url),
    new URL('../../pan/menus/FileTopbtn.vue', import.meta.url),
    new URL('../../pan/menus/FileRightMenu.vue', import.meta.url),
    new URL('../../pan/menus/DirLeftMenu.vue', import.meta.url),
    new URL('../../pan/PanRight.vue', import.meta.url)
  ]
  const source = files.map(file => readFileSync(file, 'utf8')).join('\n')
  for (const guard of ['supportsCreateFolder', 'supportsLocalUpload', 'supportsRename', 'supportsMove', 'supportsCopy', 'supportsTrashMove', 'supportsTrashPermanentDelete', 'supportsCreateShare', 'supportsShareImport']) {
    expect(source).toContain(guard)
  }
})

it('allows read-only folders to be copied into shortcuts without enabling folder moves', () => {
  const left = readFileSync(new URL('../../pan/PanLeft.vue', import.meta.url), 'utf8')
  const right = readFileSync(new URL('../../pan/PanRight.vue', import.meta.url), 'utf8')
  const dragStart = right.slice(right.indexOf('const onRowItemDragStart'), right.indexOf('const onRowItemDragEnter'))
  const folderDrop = right.slice(right.indexOf('const onRowItemDrop'), right.indexOf('const onRowItemDragEnd'))

  expect(left).toContain("@dragover='onQuickDragOver'")
  expect(left).toContain("ev.dataTransfer.dropEffect = 'copy'")
  expect(dragStart).not.toContain('!canMove.value')
  expect(folderDrop).toContain('if (!canMove.value) return')
})
