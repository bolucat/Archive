import { describe, expect, it } from 'vitest'
import { parseTorrentMeta } from './torrentMeta'

const enc = new TextEncoder()
const b = (value: string) => enc.encode(value)

describe('parseTorrentMeta', () => {
  it('parses single-file torrents', () => {
    const meta = parseTorrentMeta(b('d4:infod6:lengthi123e4:name9:movie.mkvee'))
    expect(meta).toEqual({
      name: 'movie.mkv',
      files: [{ index: 1, path: 'movie.mkv', length: 123 }]
    })
  })

  it('parses multi-file torrents with aria2 1-based indexes', () => {
    const meta = parseTorrentMeta(b('d4:infod5:filesld6:lengthi10e4:pathl5:a.txteed6:lengthi20e4:pathl3:dir5:b.txteee4:name6:folderee'))
    expect(meta.name).toBe('folder')
    expect(meta.files).toEqual([
      { index: 1, path: 'a.txt', length: 10 },
      { index: 2, path: 'dir/b.txt', length: 20 }
    ])
  })
})
