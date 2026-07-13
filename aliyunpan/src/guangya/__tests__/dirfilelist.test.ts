import { beforeAll, describe, expect, it, vi } from 'vitest'

;(globalThis as any).self = globalThis
;(globalThis as any).pinyinlite = (input: string) => input.split('').map((char) => [char])

vi.mock('../../utils/message', () => ({
  default: {
    error: vi.fn()
  }
}))

vi.mock('../../user/userdal', () => ({
  default: {
    GetUserToken: vi.fn(() => ({})),
    GetUserTokenFromDB: vi.fn(async () => ({})),
    SaveUserToken: vi.fn()
  }
}))

vi.mock('../auth', () => ({
  GUANGYA_API_URL: 'https://api.guangyapan.com',
  guangyaApiHeaders: vi.fn(() => ({})),
  refreshGuangyaAccessToken: vi.fn()
}))

let helpers: typeof import('../dirfilelist')

beforeAll(async () => {
  helpers = await import('../dirfilelist')
})

describe('Guangya dirfilelist helpers', () => {
  it('normalizes root parent ids for Guangya API calls', () => {
    expect(helpers.guangyaApiParentId('guangya_root')).toBe('')
    expect(helpers.guangyaApiParentId('0')).toBe('')
    expect(helpers.guangyaApiParentId('/')).toBe('')
    expect(helpers.guangyaApiParentId('*')).toBe('*')
    expect(helpers.guangyaApiParentId('folder-id')).toBe('folder-id')
  })

  it('reads common id, name and folder fields from Guangya items', () => {
    expect(helpers.getGuangyaFileId({ fileId: 'file-id' })).toBe('file-id')
    expect(helpers.getGuangyaFileName({ fileName: 'movie.mp4' })).toBe('movie.mp4')
    expect(helpers.isGuangyaDir({ type: 'folder' })).toBe(true)
    expect(helpers.isGuangyaDir({ name: 'movie.mp4', type: 'file' })).toBe(false)
  })

  it('maps Guangya files into the shared cloud file model with hashes', () => {
    const model = helpers.mapGuangyaFileToAliModel({
      fileId: 'file-id',
      parentId: 'parent-id',
      name: 'movie.mp4',
      size: 1048576,
      contentHash: 'md5-base64',
      updateAt: '2026-07-10T01:00:00Z',
      thumbnailUrl: 'https://thumb'
    })

    expect(model.drive_id).toBe('guangya')
    expect(model.file_id).toBe('file-id')
    expect(model.parent_file_id).toBe('parent-id')
    expect(model.ext).toBe('mp4')
    expect(model.sizeStr).toBe('1.00MB')
    expect(model.thumbnail).toBe('https://thumb')
    expect((model as any).content_hash).toBe('md5-base64')
    expect(model.description).toBe('guangya_id:file-id')
  })
})
