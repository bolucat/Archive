import { describe, expect, it } from 'vitest'

describe('TreeStore virtual directories', () => {
  it('resolves the Box recent-files node before loading its collection', async () => {
    Object.assign(globalThis, { self: globalThis })
    const { default: TreeStore } = await import('../../store/treestore')

    expect(TreeStore.GetDir('box', 'recent')).toMatchObject({
      drive_id: 'box',
      file_id: 'recent',
      parent_file_id: '',
      name: '最近文件'
    })

    expect(TreeStore.GetDirPath('box', 'recent')).toEqual([
      expect.objectContaining({
        drive_id: 'box',
        file_id: 'recent',
        parent_file_id: '',
        name: '最近文件'
      })
    ])
  })
})
