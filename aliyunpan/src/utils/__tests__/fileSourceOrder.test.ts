import { describe, expect, it } from 'vitest'
import { applyFileSourceOrder, reorderFileSourceIds } from '../fileSourceOrder'

describe('file source drag ordering', () => {
  it('moves in both directions without mutating the original order', () => {
    const ids = ['folder:a', 'server:b', 'folder:c']
    expect(reorderFileSourceIds(ids, 'folder:a', 'folder:c')).toEqual(['server:b', 'folder:c', 'folder:a'])
    expect(reorderFileSourceIds(ids, 'folder:c', 'folder:a')).toEqual(['folder:c', 'folder:a', 'server:b'])
    expect(ids).toEqual(['folder:a', 'server:b', 'folder:c'])
  })
  it('ignores missing and identical targets', () => {
    const ids = ['a', 'b']
    expect(reorderFileSourceIds(ids, 'missing', 'a')).toEqual(ids)
    expect(reorderFileSourceIds(ids, 'a', 'missing')).toEqual(ids)
    expect(reorderFileSourceIds(ids, 'a', 'a')).toEqual(ids)
    expect(reorderFileSourceIds(ids, 'a', 'a')).not.toBe(ids)
  })
  it('preserves covers and unrelated overrides while saving contiguous positions', () => {
    const overrides = { a: { cover: 'https://example.com/cover.jpg', order: 8 }, other: { order: 9 } }
    expect(applyFileSourceOrder(overrides, ['b', 'a'])).toEqual({ a: { cover: 'https://example.com/cover.jpg', order: 1 }, b: { order: 0 }, other: { order: 9 } })
    expect(overrides.a.order).toBe(8)
  })
})
