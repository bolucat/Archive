import { describe, expect, it } from 'vitest'
import { buildAriaAddOptions, shouldCheckExistingDownloadTarget } from './aria2AddOptions'

describe('buildAriaAddOptions', () => {
  it('keeps HTTP-only options on URI downloads', () => {
    expect(buildAriaAddOptions({
      gid: 'g1',
      dir: '/tmp',
      split: 4,
      referer: 'https://www.aliyundrive.com/drive',
      userAgent: 'Chrome',
      headers: ['Authorization: Bearer token'],
      outFileName: 'movie.mkv',
      sourceType: 'url'
    })).toEqual({
      gid: 'g1',
      dir: '/tmp',
      split: 4,
      out: 'movie.mkv',
      referer: 'https://www.aliyundrive.com/drive',
      'user-agent': 'Chrome',
      header: ['Authorization: Bearer token']
    })
  })

  it('does not pass HTTP-only options to local torrent tasks', () => {
    expect(buildAriaAddOptions({
      gid: 'g1',
      dir: '/tmp',
      split: 4,
      referer: 'https://www.aliyundrive.com/drive',
      userAgent: 'Chrome',
      headers: ['User-Agent: Chrome', 'Authorization: Bearer token'],
      outFileName: 'movie.mkv',
      sourceType: 'torrent'
    })).toEqual({
      gid: 'g1',
      dir: '/tmp',
      split: 4
    })
  })

  it('passes selected torrent file indexes to BT tasks', () => {
    expect(buildAriaAddOptions({
      gid: 'g1',
      dir: '/tmp',
      split: 4,
      referer: '',
      userAgent: '',
      headers: [],
      outFileName: '',
      sourceType: 'torrent',
      selectFile: '1,3'
    })).toEqual({
      gid: 'g1',
      dir: '/tmp',
      split: 4,
      'select-file': '1,3'
    })
  })

  it('allows BT tasks to resume when their target directory already exists', () => {
    expect(shouldCheckExistingDownloadTarget('magnet')).toBe(false)
    expect(shouldCheckExistingDownloadTarget('torrent')).toBe(false)
    expect(shouldCheckExistingDownloadTarget('url')).toBe(true)
  })
})
