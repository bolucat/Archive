import { describe, expect, it } from 'vitest'
import { readFileSync } from 'fs'
import { resolve } from 'path'
import { createRemoteDriveAccount } from '../remoteDriveAccount'
import type { WebDavConnectionConfig } from '../webdavClient'

const connection = (kind: 'webdav' | 'alist'): WebDavConnectionConfig => ({
  id: `${kind}-connection`,
  kind,
  name: kind === 'alist' ? '家庭 AList' : 'NAS 影视库',
  url: kind === 'alist' ? 'https://alist.example.com/dav' : 'https://dav.example.com',
  username: 'boxplayer',
  password: 'secret',
  rootPath: '/media',
  createdAt: '2026-07-22T00:00:00.000Z'
})

describe('remote drive account adapter', () => {
  it.each(['webdav', 'alist'] as const)('creates a switchable %s account and a scrapeable media root', (kind) => {
    const result = createRemoteDriveAccount(connection(kind))

    expect(result.token).toMatchObject({
      tokenfrom: kind,
      user_id: `${kind}_${kind}-connection`,
      nick_name: kind === 'alist' ? '家庭 AList' : 'NAS 影视库',
      default_drive_id: `webdav:${kind}-connection`
    })
    expect(result.folder).toMatchObject({
      fileId: '/',
      path: '/',
      userId: `${kind}-connection`,
      driveId: `webdav:${kind}-connection`,
      driveServerId: 'webdav'
    })
  })

  it('exposes WebDAV and AList from the account dialog and keeps the video page free of duplicate add dialogs', () => {
    const root = resolve(__dirname, '../../..')
    const login = readFileSync(resolve(root, 'src/user/UserLogin.vue'), 'utf8')
    const videoNav = readFileSync(resolve(root, 'src/components/MediaLibraryNav.vue'), 'utf8')

    expect(login).toContain("'webdav', 'alist'")
    expect(login).toContain('const submitRemoteDrive = async () =>')
    expect(login).toContain('连接并打开网盘')
    expect(videoNav).not.toContain('AddWebDavModal')
    expect(videoNav).not.toContain('AddAListModal')
  })
})
