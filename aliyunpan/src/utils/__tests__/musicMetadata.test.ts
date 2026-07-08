import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('axios', () => ({
  default: {
    get: vi.fn(async (url: string) => {
      if (String(url).includes('itunes.apple.com')) {
        return {
          data: {
            results: [
              {
                trackName: 'Song',
                artistName: 'Artist',
                collectionName: 'Album',
                artworkUrl100: 'https://img.example/100x100bb.jpg'
              }
            ]
          }
        }
      }
      return { data: {} }
    })
  }
}))

vi.mock('../debuglog', () => ({
  default: {
    mSaveWarning: vi.fn(),
    mSaveInfo: vi.fn()
  }
}))

const musicsdkMock = vi.hoisted(() => {
  const fetchLyricDetailed = vi.fn(async () => ({
    lyric: { lyric: '[00:01.00]hello\n[00:03.00]world' },
    debug: {
      query: { name: 'Song', singer: 'Artist' },
      matched: { source: 'wy', name: 'Song', singer: 'Artist', songmid: 1 },
      steps: [{ source: 'wy', status: 'ok', message: 'ok', lyricLength: 31 }],
      finalStatus: 'ok'
    }
  }))
  const fetchCover = vi.fn(async () => null)
  return {
    fetchLyricDetailed,
    fetchCover,
    module: () => ({
      fetchLyricDetailed,
      fetchCover
    })
  }
})

vi.mock('../module/musicsdk/index', musicsdkMock.module)
vi.mock('../../module/musicsdk/index', musicsdkMock.module)
vi.mock('/Users/gaozhangmin/aliyunpan/src/module/musicsdk/index.ts', musicsdkMock.module)

describe('musicMetadata', () => {
  beforeEach(async () => {
    musicsdkMock.fetchLyricDetailed.mockReset()
    musicsdkMock.fetchLyricDetailed.mockResolvedValue({
      lyric: { lyric: '[00:01.00]hello\n[00:03.00]world' },
      debug: {
        query: { name: 'Song', singer: 'Artist' },
        matched: { source: 'wy', name: 'Song', singer: 'Artist', songmid: 1 },
        steps: [{ source: 'wy', status: 'ok', message: 'ok', lyricLength: 31 }],
        finalStatus: 'ok'
      }
    })
    const mod = await import('../musicMetadata')
    mod.clearMusicMetaCache()
  })

  it('parses Kugou KRC millisecond timestamps returned by the SDK', async () => {
    const { parseLrc } = await import('../musicMetadata')

    expect(parseLrc('[1234,2800]hello<200,300>world\n[4567,1200]next')).toEqual([
      { time: 1.234, text: 'helloworld' },
      { time: 4.567, text: 'next' }
    ])
  })

  it('strips track numbers before splitting artist and title', async () => {
    const { guessArtistTitle } = await import('../musicMetadata')

    expect(guessArtistTitle('03. 周杰倫 - 星晴.flac')).toEqual({ artist: '周杰倫', title: '星晴' })
    expect(guessArtistTitle('12 周杰倫 - 七里香.mp3')).toEqual({ artist: '周杰倫', title: '七里香' })
  })

  it('applies global LRC offset to timed lyrics', async () => {
    const { parseLrc } = await import('../musicMetadata')

    expect(parseLrc('[offset:1500]\n[00:01.00]hello\n[00:03.00]world')).toEqual([
      { time: 2.5, text: 'hello' },
      { time: 4.5, text: 'world' }
    ])
  })

  it('keeps plain lyrics visible when the lyric source has no timeline', async () => {
    musicsdkMock.fetchLyricDetailed.mockResolvedValue({
      lyric: { lyric: '第一句歌词\n第二句歌词' },
      debug: {
        query: { name: 'Song', singer: 'Artist' },
        matched: { source: 'wy', name: 'Song', singer: 'Artist', songmid: 1 },
        steps: [{ source: 'wy', status: 'ok', message: 'ok', lyricLength: 11 }],
        finalStatus: 'ok'
      }
    })
    const { fetchMusicMetadata } = await import('../musicMetadata')

    const meta = await fetchMusicMetadata({ filename: 'Artist - Song.mp3' })

    expect(meta.lines.map((line) => line.text)).toEqual(['第一句歌词', '第二句歌词'])
    expect(meta.debug?.reason).toBe('歌词接口返回纯文本，已按行展示 2 行')
  })

  it('spreads plain lyrics across the known track duration', async () => {
    const { parseLrc } = await import('../musicMetadata')

    expect(parseLrc('第一句歌词\n第二句歌词\n第三句歌词', 90)).toEqual([
      { time: 0, text: '第一句歌词' },
      { time: 30, text: '第二句歌词' },
      { time: 60, text: '第三句歌词' }
    ])
  })

  it('records metadata source steps without turning external metadata into playable music', async () => {
    const { fetchMusicMetadata } = await import('../musicMetadata')

    const meta = await fetchMusicMetadata({ filename: 'Artist - Song.mp3' })

    expect(meta.metadataSources).toEqual(expect.arrayContaining(['filename', 'itunes:cover', 'musicsdk:lyrics']))
    expect(meta.cover).toBe('https://img.example/600x600bb.jpg')
    expect(meta.lines.map((line) => line.text)).toEqual(['hello', 'world'])
  })

  it('passes local track duration to lyric matching', async () => {
    const { fetchMusicMetadata } = await import('../musicMetadata')

    await fetchMusicMetadata({ filename: '03. Artist - Song.mp3', durationSec: 256 })

    expect(musicsdkMock.fetchLyricDetailed).toHaveBeenCalledWith({ name: 'Song', singer: 'Artist', interval: '04:16' })
  })
})
