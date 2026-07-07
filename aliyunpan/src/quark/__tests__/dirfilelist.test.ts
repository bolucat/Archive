import { describe, expect, it, vi, afterEach } from 'vitest'
import { apiQuarkDownloadUrl, apiQuarkFileList, mapQuarkFileToAliModel } from '../dirfilelist'

(globalThis as any).pinyinlite = (input: string) => input.split('').map((char) => [char])

vi.mock('../../user/userdal', () => ({
  default: {
    GetUserToken: () => ({ access_token: '__uid=u1; __kps=kps1', user_id: 'quark_u1' }),
    GetUserTokenFromDB: async () => null
  }
}))

vi.mock('../../utils/message', () => ({
  default: {
    error: vi.fn()
  }
}))

afterEach(() => vi.unstubAllGlobals())

describe('apiQuarkFileList', () => {
  it('lists one Quark folder with cookie auth and pdir_fid', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        status: 200,
        data: {
          list: [{ fid: 'f1', pdir_fid: '0', file_name: 'Movie.mkv', file_type: 1, size: 1024 }],
          metadata: { _total: 1 }
        }
      })
    })
    vi.stubGlobal('fetch', fetchMock)

    const { items, total } = await apiQuarkFileList('quark_u1', '0', 50)

    expect(items).toHaveLength(1)
    expect(total).toBe(1)
    const [url, init] = fetchMock.mock.calls[0]
    expect(String(url)).toContain('/file/sort?')
    expect(String(url)).toContain('pdir_fid=0')
    expect((init.headers as Record<string, string>).cookie).toContain('__uid=u1')
  })
})

describe('apiQuarkDownloadUrl', () => {
  it('uses Quark desktop client download parameters and user agent', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        status: 200,
        data: [{
          download_url: 'https://download.example/video.mp4',
          file_name: 'Movie.mp4',
          size: 2048
        }]
      })
    })
    vi.stubGlobal('fetch', fetchMock)

    const result = await apiQuarkDownloadUrl('quark_u1', 'fid-video')

    expect(result).toMatchObject({ url: 'https://download.example/video.mp4', name: 'Movie.mp4', size: 2048, error: '' })
    const [url, init] = fetchMock.mock.calls[0]
    expect(String(url)).toContain('/file/download?')
    expect(String(url)).toContain('sys=win32')
    expect(String(url)).toContain('ve=2.5.56')
    expect(String(url)).toContain('ut=')
    expect(String(url)).toContain('guid=')
    expect(String(url)).not.toContain('uc_param_str')
    expect(String(url)).not.toContain('__t=')
    expect(String(url)).not.toContain('__dt=')
    expect(init.method).toBe('POST')
    expect((init.headers as Record<string, string>)['user-agent']).toContain('quark-cloud-drive/2.5.56')
    expect(JSON.parse(init.body as string)).toMatchObject({ fids: ['fid-video'] })
  })

  it('returns Quark download errors from the API response', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: false,
      status: 400,
      json: async () => ({
        status: 400,
        code: 23018,
        message: 'download file size limit[fid-video]'
      })
    }))

    const result = await apiQuarkDownloadUrl('quark_u1', 'fid-video')

    expect(result.url).toBe('')
    expect(result.error).toBe('download file size limit[fid-video]')
  })
})

describe('mapQuarkFileToAliModel', () => {
  it('maps folders and files into the shared cloud file model', () => {
    const folder = mapQuarkFileToAliModel({
      fid: 'd1',
      pdir_fid: '0',
      file_name: '资料',
      file_type: 0,
      updated_at: 1710000000000
    }, 'quark', 'quark_root')

    const file = mapQuarkFileToAliModel({
      fid: 'f1',
      pdir_fid: 'd1',
      file_name: 'Movie.mkv',
      file_type: 1,
      size: 1048576,
      thumbnail: 'https://thumb'
    }, 'quark', 'd1')

    expect(folder).toMatchObject({ drive_id: 'quark', file_id: 'd1', parent_file_id: 'quark_root', isDir: true })
    expect(file).toMatchObject({ drive_id: 'quark', file_id: 'f1', parent_file_id: 'd1', isDir: false, ext: 'mkv', size: 1048576 })
    expect(file.description).toContain('quark_fid:f1')
  })
})
