import { describe, expect, it } from 'vitest'
import { mergeTreeRootsPreservingOrder } from '../../pan/treeRootMerge'

describe('mergeTreeRootsPreservingOrder', () => {
  it('keeps backup and resource roots stable when either root is refreshed', () => {
    const backup = { key: 'backup_root', title: '备份盘', version: 1 }
    const resource = { key: 'resource_root', title: '资源盘', version: 1 }
    const initial = [{ key: 'search', title: '全盘搜索', version: 1 }, backup, resource]

    const afterBackup = mergeTreeRootsPreservingOrder(initial, [{ ...backup, version: 2 }])
    expect(afterBackup.map((item) => item.key)).toEqual(['search', 'backup_root', 'resource_root'])
    expect(afterBackup[1].version).toBe(2)

    const afterResource = mergeTreeRootsPreservingOrder(afterBackup, [{ ...resource, version: 2 }])
    expect(afterResource.map((item) => item.key)).toEqual(['search', 'backup_root', 'resource_root'])
    expect(afterResource[2].version).toBe(2)
  })

  it('appends a root only when it is not already present', () => {
    expect(mergeTreeRootsPreservingOrder([{ key: 'search' }], [{ key: 'backup_root' }]).map((item) => item.key)).toEqual(['search', 'backup_root'])
  })
})
