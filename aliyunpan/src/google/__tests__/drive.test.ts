import { describe, expect, it } from 'vitest'
import { buildGoogleAuthUrl } from '../auth'
import { buildGoogleDownloadUrl, buildGoogleExportUrl, buildGoogleFilesListPath, buildGoogleSharedWithMeListPath, mapGoogleFileToAliModel, resolveGoogleTokenForRequest } from '../dirfilelist'
import { googleRootVirtualItems } from '../adapter'
import { buildGoogleContentRange, buildGoogleMultipartUploadBody } from '../upload'
import { buildGoogleChangesPath, buildGoogleStartPageTokenPath } from '../changes'
import { buildGoogleDownloadPath, buildGoogleOperationPath } from '../download'

describe('Google Drive v3 request builders', () => {
  it('uses the official root list pagination parameters', () => {
    const url = new URL(`https://example.test${buildGoogleFilesListPath('google_root', 'next', 2000)}`)
    expect(url.searchParams.get('pageSize')).toBe('1000')
    expect(url.searchParams.get('pageToken')).toBe('next')
    expect(url.searchParams.get('q')).toContain("'root' in parents")
    expect(url.searchParams.get('supportsAllDrives')).toBe('true')
  })

  it('uses the shared-with-me virtual directory query', () => {
    const url = new URL(`https://example.test${buildGoogleSharedWithMeListPath()}`)
    expect(url.searchParams.get('q')).toBe('sharedWithMe = true and trashed = false')
  })

  it('defines the shared directory as a root navigation item', () => {
    expect(googleRootVirtualItems()).toEqual([{ id: 'google_shared', name: '共享云端硬盘', mimeType: 'application/vnd.google-apps.folder' }])
  })

  it('builds download and export URLs by file type', () => {
    expect(buildGoogleDownloadUrl('file id')).toContain('alt=media')
    expect(buildGoogleDownloadUrl('file id', 'resource key')).toContain('resourceKey=resource+key')
    expect(buildGoogleExportUrl('doc', 'application/vnd.google-apps.document')).toContain('mimeType=application%2Fpdf')
    expect(buildGoogleExportUrl('form', 'application/vnd.google-apps.form')).toBe('')
  })

  it('opens folder shortcuts with their target folder ID', () => {
    ;(globalThis as any).pinyinlite = (value: string) => [[value]]
    const folder = mapGoogleFileToAliModel({ id: 'shortcut-id', name: '项目', mimeType: 'application/vnd.google-apps.shortcut', shortcutDetails: { targetId: 'target-folder-id', targetMimeType: 'application/vnd.google-apps.folder' } }, 'google', 'google_root')
    expect(folder.file_id).toBe('target-folder-id')
    expect(folder.mime_type).toBe('application/vnd.google-apps.folder')
    expect(folder.isDir).toBe(true)
  })

  it('does not borrow another Google account when the requested user is invalid', async () => {
    const token = await resolveGoogleTokenForRequest('aliyun_user', {
      getUserToken: () => ({ tokenfrom: 'aliyun', access_token: 'aliyun-access' }),
      getUserTokenFromDB: async () => ({ tokenfrom: 'google', user_id: 'google_other', access_token: 'google-access' })
    })
    expect(token).toBeUndefined()
  })

  it('uses PKCE desktop redirect and valid content range', async () => {
    const redirectUri = 'http://127.0.0.1:43127/oauth2callback'
    const url = new URL(await buildGoogleAuthUrl('client', 'a'.repeat(64), redirectUri, 'state'))
    expect(url.searchParams.get('redirect_uri')).toBe(redirectUri)
    expect(url.searchParams.get('code_challenge_method')).toBe('S256')
    expect(url.searchParams.get('state')).toBe('state')
    expect(buildGoogleContentRange(8, 4, 20)).toBe('bytes 8-11/20')
    expect(buildGoogleMultipartUploadBody('google_root', 'note.txt', 'text/plain', new Uint8Array([1]), 'test')).toBeInstanceOf(Blob)
  })

  it('uses Drive changes pagination with shared-drive context', () => {
    const start = new URL(`https://example.test${buildGoogleStartPageTokenPath('drive-1')}`)
    const changes = new URL(`https://example.test${buildGoogleChangesPath('cursor', 'drive-1')}`)
    expect(start.pathname).toBe('/changes/startPageToken')
    expect(changes.searchParams.get('corpora')).toBe('drive')
    expect(changes.searchParams.get('driveId')).toBe('drive-1')
  })

  it('uses the documented long-running download paths and resource key', () => {
    const download = new URL(`https://example.test${buildGoogleDownloadPath('file id', 'video/mp4', '', 'key')}`)
    const operation = new URL(`https://example.test${buildGoogleOperationPath('operations/download-1', 'key')}`)
    expect(download.pathname).toBe('/files/file%20id/download')
    expect(download.searchParams.get('mimeType')).toBe('video/mp4')
    expect(download.searchParams.get('resourceKey')).toBe('key')
    expect(operation.pathname).toBe('/operations/download-1')
    expect(operation.searchParams.get('resourceKey')).toBe('key')
  })
})
