import { describe, expect, it } from 'vitest'
import { BOX_DIRECT_UPLOAD_LIMIT, buildBoxAutoRenameName, buildBoxCommitBody, buildBoxContentRange, buildBoxPartDigest, buildBoxSmallUploadAttributes, buildBoxUploadSessionBody, buildBoxUploadSessionPath, getBoxRetryAfterMs, toBoxConflictBehavior } from '../upload'

describe('Box upload helpers', () => {
  it('builds small upload attributes for root and child folders', () => {
    expect(buildBoxSmallUploadAttributes('box_root', 'note.txt')).toEqual({ name: 'note.txt', parent: { id: '0' } })
    expect(buildBoxSmallUploadAttributes('123', 'note.txt')).toEqual({ name: 'note.txt', parent: { id: '123' } })
  })

  it('builds upload session path and body', () => {
    expect(buildBoxUploadSessionPath()).toBe('/files/upload_sessions')
    expect(buildBoxUploadSessionBody('box_root', 'movie.mkv', 1024)).toEqual({
      folder_id: '0',
      file_name: 'movie.mkv',
      file_size: 1024
    })
  })

  it('maps conflict behavior', () => {
    expect(toBoxConflictBehavior('auto_rename')).toBe('rename')
    expect(toBoxConflictBehavior('overwrite')).toBe('overwrite')
    expect(toBoxConflictBehavior('refuse')).toBe('refuse')
  })

  it('keeps extensions when generating an automatic conflict-free name', () => {
    expect(buildBoxAutoRenameName('movie.mkv', 2)).toBe('movie (2).mkv')
    expect(buildBoxAutoRenameName('README', 1)).toBe('README (1)')
  })

  it('uses Box required SHA-1 digest and byte ranges for chunk uploads', () => {
    expect(BOX_DIRECT_UPLOAD_LIMIT).toBe(50 * 1024 * 1024)
    expect(buildBoxPartDigest(Buffer.from('abc'))).toBe('sha=qZk+NkcGgWq6PiVxeFDCbJzQ2J0=')
    expect(buildBoxContentRange(8388608, 1024, 10000000)).toBe('bytes 8388608-8389631/10000000')
    expect(buildBoxCommitBody([{ part_id: 'part-1', offset: 0, size: 1024, sha1: 'abc' }])).toEqual({ parts: [{ part_id: 'part-1', offset: 0, size: 1024, sha1: 'abc' }] })
  })

  it('honors retry-after values while waiting for Box to finish a commit', () => {
    expect(getBoxRetryAfterMs('3')).toBe(3000)
    expect(getBoxRetryAfterMs('invalid')).toBe(1000)
  })
})
