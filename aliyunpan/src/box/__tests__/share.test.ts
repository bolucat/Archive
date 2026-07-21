import { describe, expect, it } from 'vitest'
import {
  buildBoxSharedLinkBody,
  buildBoxSharedLinkPath,
  buildBoxSharedLinkUpdateBody,
  decodeBoxShareId,
  encodeBoxShareId,
  mapBoxSharedLinkToAliShareItem
} from '../share'

describe('Box share helpers', () => {
  it('builds open shared link body', () => {
    expect(buildBoxSharedLinkBody()).toEqual({
      shared_link: {
        access: 'open',
        permissions: { can_download: true }
      }
    })
  })

  it('maps Box shared link into app share item', () => {
    const item = mapBoxSharedLinkToAliShareItem({
      id: 'file-id',
      type: 'file',
      name: 'Demo.mp4',
      shared_link: { url: 'https://share', download_url: 'https://download', access: 'open' }
    }, 'box', ['file-id'], 'Demo')

    expect(item.share_id).toBe('box:file:file-id')
    expect(item.share_url).toBe('https://share')
    expect(item.share_name).toBe('Demo')
    expect(item.file_id_list).toEqual(['file-id'])
  })

  it('uses the folders endpoint for folder shared links', () => {
    expect(buildBoxSharedLinkPath('folder', 'folder-id')).toBe('/folders/folder-id')
    expect(buildBoxSharedLinkPath('file', 'file-id')).toBe('/files/file-id')
  })

  it('builds Box shared-link update settings', () => {
    expect(buildBoxSharedLinkUpdateBody('2026-06-01T08:00:00.000Z', '1234')).toEqual({
      shared_link: {
        access: 'open',
        permissions: { can_download: true },
        password: '1234',
        unshared_at: '2026-06-01T08:00:00.000Z'
      }
    })
    expect(buildBoxSharedLinkUpdateBody('', '')).toEqual({
      shared_link: {
        access: 'open',
        permissions: { can_download: true },
        password: null,
        unshared_at: null
      }
    })
  })

  it('encodes and decodes the Box item target for share management', () => {
    const shareId = encodeBoxShareId('folder', 'folder-id')
    expect(shareId).toBe('box:folder:folder-id')
    expect(decodeBoxShareId(shareId)).toEqual({ type: 'folder', id: 'folder-id' })
    expect(decodeBoxShareId('https://share')).toBeUndefined()
  })
})
