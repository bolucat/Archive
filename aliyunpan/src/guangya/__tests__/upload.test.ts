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

  it('posts a direct upload using the returned OSS form policy', async () => {
    guangyaRequest
      .mockResolvedValueOnce({ data: { taskId: 'task-1', objectPath: 'folder/file.txt', params: { url: 'https://oss.example.com', multipart: { OSSAccessKeyId: 'key', Signature: 'signature', key: 'folder/file.txt', policy: 'policy', 'x:user_data': 'user-data' } } } })
      .mockResolvedValueOnce({ data: { message: '文件上传中' } })
      .mockResolvedValueOnce({ data: { fileId: 'file-1' } })
    const fetchMock = vi.fn().mockResolvedValue({ ok: true })
    vi.stubGlobal('fetch', fetchMock)

    await expect(apiGuangyaUploadBuffer('user', 'root', 'file.txt', Buffer.from('hello'))).resolves.toEqual({ file_id: 'file-1', error: '' })

    expect(guangyaRequest).toHaveBeenNthCalledWith(1, 'user', '/nd.bizuserres.s/v1/get_res_center_token', {
      capacity: 1,
      name: 'file.txt',
      parentId: 'root',
      res: { fileSize: 5, md5: 'XUFAKrxLKna5cZ2REBfFkg==' }
    })

    expect(fetchMock).toHaveBeenCalledWith('https://oss.example.com', expect.objectContaining({ method: 'POST' }))
    const form = fetchMock.mock.calls[0][1].body as FormData
    expect(form.get('key')).toBe('folder/file.txt')
    expect(form.get('OSSAccessKeyId')).toBe('key')
    expect(form.get('Signature')).toBe('signature')
    expect(form.get('policy')).toBe('policy')
    expect(form.get('x:user_data')).toBe('user-data')
  })
})
