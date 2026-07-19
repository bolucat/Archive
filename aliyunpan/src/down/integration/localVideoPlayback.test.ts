import path from 'path'
import { describe, expect, it } from 'vitest'
import { buildLocalVideoPage, isLocalVideoPath } from './localVideoPlayback'

describe('completed local video playback', () => {
  it('recognizes video files without treating folders and subtitles as video', () => {
    expect(isLocalVideoPath('/downloads/Movie.MKV')).toBe(true)
    expect(isLocalVideoPath('/downloads/episode.m2ts')).toBe(true)
    expect(isLocalVideoPath('/downloads/Movie.srt')).toBe(false)
    expect(isLocalVideoPath('/downloads/Show.S01')).toBe(false)
  })

  it('builds PageVideo data for the app local player', () => {
    const filePath = path.join('/downloads', 'Movie.2026.mp4')
    expect(buildLocalVideoPage(filePath)).toMatchObject({
      user_id: 'local',
      drive_id: 'local',
      file_id: filePath,
      parent_file_id: '/downloads',
      parent_file_name: 'downloads',
      file_name: 'Movie.2026.mp4',
      html: 'Movie.2026.mp4'
    })
  })
})
