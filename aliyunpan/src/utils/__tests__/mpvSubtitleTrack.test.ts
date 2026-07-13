import { describe, expect, it } from 'vitest'
import { getAutoSubtitleTrackId } from '../mpvSubtitleTrack'

describe('getAutoSubtitleTrackId', () => {
  const tracks = [
    { id: 1, type: 'video' },
    { id: 2, type: 'audio' },
    { id: 3, type: 'sub', language: 'chi' },
    { id: 4, type: 'sub', language: 'eng' }
  ]

  it('selects the first embedded subtitle when MPV has not selected one', () => {
    expect(getAutoSubtitleTrackId(tracks, -1, false)).toBe(3)
  })

  it('does not replace MPV-selected or external subtitles', () => {
    expect(getAutoSubtitleTrackId(tracks, 4, false)).toBeUndefined()
    expect(getAutoSubtitleTrackId(tracks, -1, true)).toBeUndefined()
  })
})
