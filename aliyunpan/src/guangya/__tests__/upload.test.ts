import { afterEach, describe, expect, it, vi } from 'vitest'

const { guangyaRequest } = vi.hoisted(() => ({ guangyaRequest: vi.fn() }))

vi.mock('../dirfilelist', () => ({
  guangyaApiParentId: (value: string) => value,
  guangyaRequest
}))

import { apiGuangyaUploadBuffer, apiGuangyaUploadInfo } from '../upload'

afterEach(() => {
  vi.resetAllMocks()
  vi.unstubAllGlobals()
})

describe('Guangya OSS upload', () => {
  it('keeps polling when the upload task is still processing', async () => {
    guangyaRequest.mockResolvedValueOnce({ data: { status: 'processing' } })

    await expect(apiGuangyaUploadInfo('user', 'task-1')).resolves.toMatchObject({ fileId: '', uploading: true, error: '' })
  })

  it('signs a direct upload with the required OSS date, token, and content MD5 headers', async () => {
    guangyaRequest
      .mockResolvedValueOnce({ data: { taskId: 'task-1', fullEndPoint: 'https://oss.example.com', bucketName: 'bucket', objectPath: 'folder/file.txt', creds: { accessKeyID: 'key', secretAccessKey: 'secret', sessionToken: 'sts-token' } } })
      .mockResolvedValueOnce({ data: { message: '文件上传中' } })
      .mockResolvedValueOnce({ data: { fileId: 'file-1' } })
    const fetchMock = vi.fn().mockResolvedValue({ ok: true })
    vi.stubGlobal('fetch', fetchMock)

    await expect(apiGuangyaUploadBuffer('user', 'root', 'file.txt', Buffer.from('hello'))).resolves.toEqual({ file_id: 'file-1', error: '' })

    const headers = fetchMock.mock.calls[0][1].headers
    expect(headers['x-oss-date']).toBeTruthy()
    expect(headers['x-oss-security-token']).toBe('sts-token')
    expect(headers['Content-MD5']).toBe('XUFAKrxLKna5cZ2REBfFkg==')
    expect(headers.Authorization).toMatch(/^OSS key:/)
  })
})
