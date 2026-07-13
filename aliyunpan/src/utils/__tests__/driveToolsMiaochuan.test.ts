import { describe, expect, it } from 'vitest'
import { normalizeMiaochuanMd5, normalizeMiaochuanPayload } from '../drive-tools/miaochuan'

describe('drive-tools miaochuan', () => {
  it('normalizes hex, base64 md5, gcid and nested payloads', () => {
    expect(normalizeMiaochuanMd5('CY9rzUYh03PK3k6DJie09g==')).toBe('098f6bcd4621d373cade4e832627b4f6')

    const result = normalizeMiaochuanPayload({
      files: [
        { path: 'Movie/a.mp4', size: 123, md5: '098f6bcd4621d373cade4e832627b4f6', provider: 'quark' },
        { filePath: '/Movie/b.mp4', fileSize: '456', gcid: '0123456789abcdef0123456789abcdef01234567', sourceProvider: 'xunlei' },
        { name: 'bad.mp4', size: 789 }
      ]
    })

    expect(result.files).toHaveLength(2)
    expect(result.files[0]).toMatchObject({ path: 'Movie/a.mp4', name: 'a.mp4', size: 123, md5: '098f6bcd4621d373cade4e832627b4f6' })
    expect(result.files[1]).toMatchObject({ path: 'Movie/b.mp4', name: 'b.mp4', size: 456, gcid: '0123456789abcdef0123456789abcdef01234567' })
    expect(result.errors).toHaveLength(1)
  })

  it('reports invalid json without throwing', () => {
    const result = normalizeMiaochuanPayload('{')
    expect(result.files).toHaveLength(0)
    expect(result.errors[0]).toContain('JSON 解析失败')
  })
})
