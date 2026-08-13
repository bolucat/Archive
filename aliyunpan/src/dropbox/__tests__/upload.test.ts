import { describe, expect, it, vi } from 'vitest'

vi.mock('../dirfilelist', () => ({ getDropboxToken: vi.fn() }))
vi.mock('../filecmd', () => ({
  resolveDropboxCommandPath: (parentId: string, description = '') => parentId === 'dropbox_root' ? '' : description || parentId
}))

import {
  buildDropboxCommitInfo,
  buildDropboxUploadPath,
  buildDropboxUploadSessionCursor,
  apiDropboxUploadBuffer,
  parseDropboxContentError,
  toDropboxWriteMode
} from '../upload'

describe('Dropbox upload helpers', () => {
  it('builds upload paths from root and nested parent folders', () => {
    expect(buildDropboxUploadPath('dropbox_root', 'movie.mkv')).toBe('/movie.mkv')
    expect(buildDropboxUploadPath('/Movies', 'movie.mkv')).toBe('/Movies/movie.mkv')
    expect(buildDropboxUploadPath('/Movies', 'Season 1/E01.mkv')).toBe('/Movies/Season 1/E01.mkv')
    expect(buildDropboxUploadPath('id:folder', 'note.txt', '/Movies')).toBe('/Movies/note.txt')
  })

  it('maps app conflict modes to Dropbox write modes', () => {
    expect(toDropboxWriteMode('overwrite')).toEqual({ mode: 'overwrite', autorename: false })
    expect(toDropboxWriteMode('ignore')).toEqual({ mode: 'overwrite', autorename: false })
    expect(toDropboxWriteMode('auto_rename')).toEqual({ mode: 'add', autorename: true })
    expect(toDropboxWriteMode('refuse')).toEqual({ mode: 'add', autorename: false })
  })

  it('builds commit info for Dropbox upload and upload session finish', () => {
    expect(buildDropboxCommitInfo('/Movies/movie.mkv', 'auto_rename')).toEqual({
      path: '/Movies/movie.mkv',
      mode: 'add',
      autorename: true,
      mute: false,
      strict_conflict: false
    })
  })

  it('builds upload session cursors with the current offset', () => {
    expect(buildDropboxUploadSessionCursor('session-id', 1024)).toEqual({
      session_id: 'session-id',
      offset: 1024
    })
  })

  it('exposes a buffer upload helper for creating text files', () => {
    expect(typeof apiDropboxUploadBuffer).toBe('function')
  })

  it('explains folder and file name conflicts from Dropbox', () => {
    expect(parseDropboxContentError('{"error_summary":"path/conflict/folder/..."}', '上传失败')).toBe('同名文件夹已存在，请更换文件名')
    expect(parseDropboxContentError('{"error_summary":"path/conflict/file/..."}', '上传失败')).toBe('同名文件已存在，请更换文件名')
  })
})
