import { describe, expect, it } from 'vitest'
import { buildBoxRepresentationInfoUrl, buildBoxThumbnailDataUrl, buildBoxThumbnailUrl } from '../thumbnail'

describe('Box thumbnail helpers', () => {
  it('builds non-paged representation content and info urls', () => {
    const template = 'https://public.boxcloud.com/api/2.0/internal_files/123/versions/456/representations/jpg_320x320/content/{+asset_path}'
    expect(buildBoxThumbnailUrl(template)).toBe('https://public.boxcloud.com/api/2.0/internal_files/123/versions/456/representations/jpg_320x320/content/')
    expect(buildBoxRepresentationInfoUrl(template)).toBe('https://api.box.com/2.0/internal_files/123/versions/456/representations/jpg_320x320')
  })

  it('converts protected thumbnail bytes into an image data url', () => {
    expect(buildBoxThumbnailDataUrl(new Uint8Array([65, 66, 67]), 'image/jpeg')).toBe('data:image/jpeg;base64,QUJD')
  })
})
