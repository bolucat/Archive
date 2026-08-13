import { describe, expect, it, vi } from 'vitest'
import {
  buildSubtitleSearchUrl,
  formatSubtitleDownloadCount,
  getSubtitleDownload,
  getSubtitleExtension,
  searchSubtitles
} from '../subtitleApi'

describe('subtitleApi', () => {
  it('builds search urls for text query and tmdb id', () => {
    expect(buildSubtitleSearchUrl('变形金刚', 'zh-cn')).toContain('query=%E5%8F%98%E5%BD%A2%E9%87%91%E5%88%9A')
    expect(buildSubtitleSearchUrl('12345', 'en')).toContain('tmdb_id=12345')
  })

  it('maps and sorts OpenSubtitles search results', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        data: [{
          id: 'a',
          attributes: {
            release: 'low',
            language: 'en',
            new_download_count: 2,
            files: [{ file_id: 11, file_name: 'low.srt' }]
          }
        }, {
          id: 'b',
          attributes: {
            release: 'high',
            language: 'zh-cn',
            new_download_count: 100,
            files: [{ file_id: 12, file_name: 'high.ass' }]
          }
        }]
      })
    })

    const results = await searchSubtitles('movie', 'zh-cn', fetchMock as any)

    expect(fetchMock).toHaveBeenCalledWith(expect.stringContaining('/subtitles?'), expect.objectContaining({
      headers: expect.objectContaining({ 'Api-Key': expect.any(String) })
    }))
    expect(results.map((item) => item.name)).toEqual(['high', 'low'])
    expect(results.map((item) => item.format)).toEqual(['ass', 'srt'])
  })

  it('preserves the subtitle file format from the search result', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        data: [{
          id: 'ass',
          attributes: {
            language: 'zh-cn',
            files: [{ file_id: 14, file_name: 'episode.zh.ass' }]
          }
        }]
      })
    })

    const [result] = await searchSubtitles('movie', 'zh-cn', fetchMock as any)

    expect(result.format).toBe('ass')
  })

  it('returns every downloadable file so format filters do not hide ASS alternatives', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        data: [{
          id: 'multi',
          attributes: {
            release: 'episode',
            language: 'en',
            files: [
              { file_id: 21, file_name: 'episode.en.srt' },
              { file_id: 22, file_name: 'episode.en.ass' },
              { file_id: 23, file_name: 'episode.en.sub' }
            ]
          }
        }]
      })
    })

    const results = await searchSubtitles('movie', 'en', fetchMock as any)

    expect(results.map((item) => [item.fileId, item.format])).toEqual([[21, 'srt'], [22, 'ass'], [23, 'unknown']])
  })

  it('loads download details and derives subtitle extension', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ file_name: 'movie.zh.ass', link: 'https://download.test/sub.ass' })
    })

    const result = await getSubtitleDownload(42, fetchMock as any)

    expect(fetchMock).toHaveBeenCalledWith(expect.stringContaining('/download'), expect.objectContaining({
      method: 'POST',
      body: JSON.stringify({ file_id: 42 })
    }))
    expect(result.fileName).toBe('movie.zh.ass')
    expect(getSubtitleExtension(result.fileName)).toBe('ass')
    expect(formatSubtitleDownloadCount(1500)).toBe('1.5K')
  })
})
