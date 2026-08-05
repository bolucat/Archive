import { describe, expect, it } from 'vitest'
import { buildDropboxThumbnailArg, buildDropboxThumbnailBatchBody, buildDropboxThumbnailDataUrl } from '../thumbnail'

describe('Dropbox thumbnail helpers', () => {
  it('builds get_thumbnail_v2 arg for a file id', () => {
    expect(buildDropboxThumbnailArg('id:file', 'png', 'w256h256')).toEqual({
      resource: {
        '.tag': 'path',
        path: 'id:file'
      },
      format: 'png',
      size: 'w256h256',
      mode: 'strict'
    })
  })

  it('converts thumbnail bytes into an image data url', () => {
    expect(buildDropboxThumbnailDataUrl(new Uint8Array([65, 66, 67]), 'jpeg')).toBe('data:image/jpeg;base64,QUJD')
  })

  it('batches thumbnail requests using the official 25-entry argument shape', () => {
    expect(buildDropboxThumbnailBatchBody(['id:one', 'id:two'], 'jpeg', 'w256h256')).toEqual({
      entries: [
        { path: 'id:one', format: 'jpeg', size: 'w256h256', mode: 'strict', quality: 'quality_80' },
        { path: 'id:two', format: 'jpeg', size: 'w256h256', mode: 'strict', quality: 'quality_80' }
      ]
    })
  })
})
