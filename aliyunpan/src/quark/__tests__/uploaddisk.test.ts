import { afterEach, describe, expect, it, vi } from 'vitest'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { open } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import QuarkUploadDisk from '../uploaddisk'

vi.hoisted(() => {
  ;(globalThis as any).self = globalThis
})

vi.mock('../../user/userdal', () => ({
  default: {
    GetUserToken: () => ({ access_token: '__uid=u1; __kps=kps1', user_id: 'quark_u1', tokenfrom: 'quark' }),
    GetUserTokenFromDB: async () => ({ access_token: '__uid=u1; __kps=kps1', user_id: 'quark_u1', tokenfrom: 'quark' })
  }
}))

vi.mock('../../utils/message', () => ({ default: { error: vi.fn() } }))

vi.mock('../../aliapi/uploaddisk', () => ({ default: { RecordUploadProgress: vi.fn() } }))

vi.mock('../../utils/filehelper', () => ({
  OpenFileHandle: async (filePath: string) => ({ handle: await open(filePath, 'r'), error: '' })
}))

vi.mock('../../utils/format', () => ({ Sleep: async () => {} }))

afterEach(() => vi.unstubAllGlobals())

describe('QuarkUploadDisk', () => {
  it('uses Quark multipart auth contexts and completes the upload', async () => {
    const directory = mkdtempSync(join(tmpdir(), 'boxplayer-quark-upload-'))
    const content = Buffer.alloc(1024 * 1024 + 9, 1)
    writeFileSync(join(directory, 'sample.txt'), content)
    const fetchMock = vi.fn()
      .mockResolvedValueOnce({ ok: true, json: async () => ({ code: 0, data: { task_id: 'task-1', auth_info: 'auth-info', upload_id: 'upload-1', obj_key: 'object-1', bucket: 'ul-zb', upload_url: 'https://pds.quark.cn', callback: { callbackUrl: 'https://callback.example' }, part_size: 1024 * 1024 } }) })
      .mockResolvedValueOnce({ ok: true, json: async () => ({ code: 0, data: { auth_key: 'put-signature' } }) })
      .mockResolvedValueOnce({ ok: true, headers: new Headers({ etag: 'part-etag' }) })
      .mockResolvedValueOnce({ ok: true, json: async () => ({ code: 0, data: { auth_key: 'put-signature-2' } }) })
      .mockResolvedValueOnce({ ok: true, headers: new Headers({ etag: 'part-etag-2' }) })
      .mockResolvedValueOnce({ ok: true, json: async () => ({ code: 0, data: { finish: false } }) })
      .mockResolvedValueOnce({ ok: true, json: async () => ({ code: 0, data: { auth_key: 'post-signature' } }) })
      .mockResolvedValueOnce({ status: 200, ok: true, headers: new Headers() })
      .mockResolvedValueOnce({ ok: true, json: async () => ({ code: 0, data: { fid: 'uploaded-file' } }) })
    vi.stubGlobal('fetch', fetchMock)

    const fileui: any = {
      user_id: 'quark_u1',
      parent_file_id: 'quark_root',
      localFilePath: directory,
      UploadID: 1,
      IsRunning: true,
      File: { partPath: 'sample.txt', name: 'sample.txt', size: content.length },
      Info: { uploadState: '' }
    }

    await expect(QuarkUploadDisk.UploadOneFile(fileui)).resolves.toBe('success')
    expect(fileui.File).toMatchObject({ uploaded_file_id: 'uploaded-file', uploaded_is_rapid: false })
    expect(String(fetchMock.mock.calls[0][0])).toContain('/file/upload/pre?')
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toMatchObject({ pdir_fid: '0', file_name: 'sample.txt', size: content.length, parallel_upload: true })
    expect(String(fetchMock.mock.calls[1][0])).toContain('/file/upload/auth?')
    expect(String(fetchMock.mock.calls[2][0])).toContain('https://ul-zb.pds.quark.cn/object-1?partNumber=1')
    expect(JSON.parse(fetchMock.mock.calls[3][1].body).auth_meta).toContain('X-Oss-Hash-Ctx:')
    expect((fetchMock.mock.calls[4][1].headers as Record<string, string>)['X-Oss-Hash-Ctx']).toBeTruthy()
    expect(String(fetchMock.mock.calls[5][0])).toContain('/file/update/hash?')
    expect(String(fetchMock.mock.calls[8][0])).toContain('/file/upload/finish?')
  })
})
