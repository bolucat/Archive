import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.hoisted(() => {
  ;(globalThis as any).self = globalThis
})

import usePanFileStore from '../../pan/panfilestore'

const item = (file_id: string, name: string, size: number, time: number, isDir = false) => ({ file_id, name, size, time, isDir }) as any

describe('cloud file list sorting', () => {
  beforeEach(() => setActivePinia(createPinia()))

  it.each([
    ['name asc', ['dir-a', 'dir-z', 'file-a', 'file-z']],
    ['size desc', ['dir-z', 'dir-a', 'file-z', 'file-a']],
    ['updated_at asc', ['dir-z', 'dir-a', 'file-a', 'file-z']]
  ])('sorts ordinary provider results by %s', (order, expected) => {
    const store = usePanFileStore()
    store.ListOrderKey = order
    store.ListDataRaw = [
      item('file-z', 'Zulu', 80, 80),
      item('dir-a', 'Folder A', 10, 20, true),
      item('file-a', 'Alpha', 20, 30),
      item('dir-z', 'Folder Z', 50, 10, true)
    ]

    store.mRefreshListDataShow(true)

    expect(store.ListDataShow.map(file => file.file_id)).toEqual(expected)
  })
})
