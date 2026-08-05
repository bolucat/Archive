import { describe, expect, it } from 'vitest'
import { buildDropboxSaveUrlBody } from '../offline'

describe('Dropbox offline download helpers', () => {
  it('creates an HTTP URL save request for the selected Dropbox path', () => {
    expect(buildDropboxSaveUrlBody('/Downloads/video.mp4', 'https://example.com/video.mp4')).toEqual({
      path: '/Downloads/video.mp4',
      url: 'https://example.com/video.mp4'
    })
  })
})
