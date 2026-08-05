import { describe, expect, it } from 'vitest'
import { buildDropboxZipDownloadHeaders } from '../zip'

describe('Dropbox ZIP download helpers', () => {
  it('builds the content-download headers for one folder', () => {
    expect(buildDropboxZipDownloadHeaders('token', 'id:folder')).toEqual({
      Authorization: 'Bearer token',
      'Dropbox-API-Arg': '{"path":"id:folder"}'
    })
  })
})
