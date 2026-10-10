import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

describe('Aliyun upload name conflicts (BP-000110)', () => {
  beforeEach(() => {
    ;(globalThis as any).self = globalThis
    setActivePinia(createPinia())
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('does not treat a pre-hash conflict as an uploaded file', async () => {
    const { default: AliHttp } = await import('../alihttp')
    const { default: AliUpload } = await import('../upload')
    const post = vi.spyOn(AliHttp, 'Post').mockResolvedValue({ code: 200, header: '', body: { file_id: 'old-file', exist: true } })

    const result = await AliUpload.UploadCreatFileWithPreHash('user', 'drive', 'root', 'movie.mkv', 10, 'prehash', 'auto_rename')

    expect(result.errormsg).toBe('PreHashMatched')
    expect(result.isexist).toBe(false)
    expect(post).toHaveBeenCalledTimes(1)
    expect(post.mock.calls[0][1]).toMatchObject({ name: 'movie.mkv', check_name_mode: 'auto_rename' })
  })

  it('creates a distinct file when auto_rename still returns the old file', async () => {
    const { default: AliHttp } = await import('../alihttp')
    const { default: AliUpload } = await import('../upload')
    const post = vi.spyOn(AliHttp, 'Post')
      .mockResolvedValueOnce({ code: 200, header: '', body: { file_id: 'old-file', exist: true } })
      .mockResolvedValueOnce({ code: 200, header: '', body: { file_id: 'new-file', exist: false, rapid_upload: false, upload_id: 'new-upload', part_info_list: [{ upload_url: 'https://example.test/part', part_number: 1 }] } })

    const result = await AliUpload.UploadCreatFileWithFolders('user', 'drive', 'root', 'movie.mkv', 10, 'hash', 'proof', 'auto_rename')

    expect(result).toMatchObject({ file_id: 'new-file', upload_id: 'new-upload', isexist: false, errormsg: '' })
    expect(post).toHaveBeenCalledTimes(2)
    expect(post.mock.calls[0][1]).toMatchObject({ name: 'movie.mkv', check_name_mode: 'auto_rename' })
    expect(post.mock.calls[1][1]).toMatchObject({ check_name_mode: 'refuse', content_hash: 'HASH' })
    expect((post.mock.calls[1][1] as any).name).toMatch(/^movie_[a-z0-9]+_[a-z0-9]+\.mkv$/)
    expect(post.mock.calls.some(([url]) => String(url).includes('recyclebin'))).toBe(false)
  })

  it('retries a server AlreadyExists response with a distinct name', async () => {
    const { default: AliHttp } = await import('../alihttp')
    const { default: AliUpload } = await import('../upload')
    const post = vi.spyOn(AliHttp, 'Post')
      .mockResolvedValueOnce({ code: 409, header: '', body: { code: 'AlreadyExists' } })
      .mockResolvedValueOnce({ code: 200, header: '', body: { file_id: 'new-file', exist: false, rapid_upload: true, upload_id: '', part_info_list: [] } })

    const result = await AliUpload.UploadCreatFileWithFolders('user', 'drive', 'root', 'movie.mkv', 10, 'hash', 'proof', 'auto_rename')

    expect(result).toMatchObject({ file_id: 'new-file', israpid: true, isexist: false, errormsg: '' })
    expect(post).toHaveBeenCalledTimes(2)
    expect(post.mock.calls[1][1]).toMatchObject({ check_name_mode: 'refuse' })
  })

  it('does not delete either file if the distinct-name retry also conflicts', async () => {
    const { default: AliHttp } = await import('../alihttp')
    const { default: AliUpload } = await import('../upload')
    const post = vi.spyOn(AliHttp, 'Post')
      .mockResolvedValueOnce({ code: 200, header: '', body: { file_id: 'old-file', exist: true } })
      .mockResolvedValueOnce({ code: 409, header: '', body: { code: 'AlreadyExists' } })

    const result = await AliUpload.UploadCreatFileWithFolders('user', 'drive', 'root', 'movie.mkv', 10, 'hash', 'proof', 'auto_rename')

    expect(result.errormsg).toBe('AlreadyExists')
    expect(result.isexist).toBe(false)
    expect(post).toHaveBeenCalledTimes(2)
    expect(post.mock.calls.some(([url]) => String(url).includes('recyclebin'))).toBe(false)
  })
})
