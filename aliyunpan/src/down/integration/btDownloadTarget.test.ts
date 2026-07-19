import path from 'path'
import { describe, expect, it } from 'vitest'
import { buildBtControlFileCandidates, isBtContentComplete, resolveBtDownloadTarget, resolveDownloadOpenPath, resolveFollowedBtGid, resolveLegacyMagnetPath } from './btDownloadTarget'

describe('BT download target', () => {
  it('uses the real aria2 file path for a single-file magnet task', () => {
    const target = resolveBtDownloadTarget({
      dir: '/downloads',
      files: [{ path: '/downloads/Real.Movie.2026.mkv' }],
      bittorrent: { info: { name: 'Real.Movie.2026.mkv' } }
    })

    expect(target).toEqual({
      localFilePath: path.join('/downloads', 'Real.Movie.2026.mkv'),
      name: 'Real.Movie.2026.mkv',
      isDir: false
    })
  })

  it('uses the torrent root directory for a multi-file magnet task', () => {
    const target = resolveBtDownloadTarget({
      dir: '/downloads',
      files: [
        { path: '/downloads/Show.S01/Show.S01E01.mkv' },
        { path: '/downloads/Show.S01/Show.S01E02.mkv' }
      ],
      bittorrent: { info: { name: 'Show.S01' } }
    })

    expect(target).toEqual({
      localFilePath: path.join('/downloads', 'Show.S01'),
      name: 'Show.S01',
      isDir: true
    })
  })

  it('prefers the recorded real path when opening a completed download', () => {
    expect(resolveDownloadOpenPath({
      DownSavePath: '/downloads',
      name: '搜索结果标题',
      localFilePath: '/downloads/Real.Movie.2026.mkv'
    })).toBe(path.join('/downloads', 'Real.Movie.2026.mkv'))
  })

  it('follows aria2 magnet metadata tasks to the real BT gid', () => {
    expect(resolveFollowedBtGid({ followedBy: ['real-bt-gid'] })).toBe('real-bt-gid')
    expect(resolveFollowedBtGid({ followedBy: [] })).toBe('')
  })

  it('recovers old completed task paths from the magnet display name', () => {
    expect(resolveLegacyMagnetPath('/downloads', 'magnet:?xt=urn:btih:abc&dn=Real.Movie.2026.mkv')).toBe(path.join('/downloads', 'Real.Movie.2026.mkv'))
    expect(resolveLegacyMagnetPath('/downloads', 'https://example.com/file')).toBe('')
  })

  it('does not treat completed magnet metadata as completed BT content', () => {
    expect(isBtContentComplete({
      totalLength: '7802619820',
      completedLength: '81920',
      files: [{ path: '/downloads/movie.mkv', selected: 'true', length: '7802619820', completedLength: '0' }]
    })).toBe(false)
  })

  it('requires every selected BT file to be fully downloaded', () => {
    expect(isBtContentComplete({
      totalLength: '110',
      completedLength: '110',
      files: [
        { path: '/downloads/movie.mkv', selected: 'true', length: '100', completedLength: '100' },
        { path: '/downloads/movie.srt', selected: 'true', length: '10', completedLength: '10' },
        { path: '/downloads/sample.mkv', selected: 'false', length: '20', completedLength: '0' }
      ]
    })).toBe(true)
  })

  it('builds control-file candidates for recovering incorrectly completed magnets', () => {
    expect(buildBtControlFileCandidates({
      DownSavePath: '/downloads',
      name: '搜索标题',
      localFilePath: '/downloads/Real.Movie'
    }, 'magnet:?xt=urn:btih:abc&dn=Real.Movie')).toEqual([
      path.join('/downloads', 'Real.Movie') + '.aria2'
    ])
  })
})
