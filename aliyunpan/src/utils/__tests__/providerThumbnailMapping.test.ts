import { describe, expect, it, vi } from 'vitest'

vi.hoisted(() => {
  ;(globalThis as any).self = globalThis
  ;(globalThis as any).pinyinlite = (input: string) => input.split('').map((char) => [char])
})

vi.mock('../../utils/message', () => ({ default: { error: vi.fn() } }))
vi.mock('../../utils/debuglog', () => ({ default: { mSaveWarning: vi.fn() } }))
vi.mock('../../cloud123/auth', () => ({ getCloud123Token: vi.fn() }))
vi.mock('../../cloud115/auth', () => ({ getDrive115Token: vi.fn() }))

import { mapCloud123FileToAliModel } from '../../cloud123/dirfilelist'
import { mapDrive115FileToAliModel, mapDrive115SearchToAliModel } from '../../cloud115/dirfilelist'
import { mapOneDriveItemToAliModel } from '../../onedrive/dirfilelist'

describe('provider thumbnail mapping', () => {
  it('preserves thumbnail fields returned by 123 Drive', () => {
    const item = mapCloud123FileToAliModel({ fileId: 1, filename: 'clip.mp4', parentFileId: 0, type: 0, size: 1, category: 0, status: 0, trashed: 0, previewUrl: 'https://thumb.example/123.jpg' })

    expect(item.thumbnail).toBe('https://thumb.example/123.jpg')
  })

  it('preserves thumbnail URLs returned by 115 list and search responses', () => {
    expect(mapDrive115FileToAliModel({ fid: '1', pid: '0', fc: 1, fn: 'clip.mp4', thumbnail_url: 'https://thumb.example/115.jpg' }, 'drive115').thumbnail).toBe('https://thumb.example/115.jpg')
    expect(mapDrive115SearchToAliModel({ file_id: '1', parent_id: '0', file_name: 'clip.mp4', file_category: '1', thumb: 'https://thumb.example/115-search.jpg' }, 'drive115').thumbnail).toBe('https://thumb.example/115-search.jpg')
  })

  it('prefers the largest OneDrive thumbnail for the large-image view', () => {
    const item = mapOneDriveItemToAliModel({ id: '1', name: 'photo.jpg', size: 1, thumbnails: [{ small: { url: 'https://thumb.example/small.jpg' }, medium: { url: 'https://thumb.example/medium.jpg' }, large: { url: 'https://thumb.example/large.jpg' } }] }, 'onedrive', 'root')

    expect(item.thumbnail).toBe('https://thumb.example/large.jpg')
  })
})
