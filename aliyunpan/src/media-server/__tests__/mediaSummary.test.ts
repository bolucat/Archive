import { describe, expect, it } from 'vitest'
import { mediaFileSummary } from '../mediaSummary'
import type { MediaServerMediaInfoCard } from '../../types/mediaServerContent'

const card = (kind: MediaServerMediaInfoCard['kind'], values: Record<string, string>, selected = false): MediaServerMediaInfoCard => ({
  id: kind, kind, title: kind, selected, rows: Object.entries(values).map(([label, value]) => ({ label, value }))
})

describe('media file summary', () => {
  it('shows the file size and all video/audio summary parameters', () => {
    expect(mediaFileSummary('2026-01-01  3.4GB', [
      card('video', { Codec: 'h264', 分辨率: '1920x1080', 比特率: '8.2Mbps', 帧率: '29.970' }),
      card('audio', { Codec: 'aac', Channels: '2' })
    ])).toBe('3.4 GB   H.264   (1080p)   AAC 2.0   8.2 Mbps   29.97 fps')
  })
  it('uses the selected audio stream and preserves explicit channel layout', () => {
    expect(mediaFileSummary(undefined, [card('audio', { Codec: 'aac', Channels: '2' }), card('audio', { Codec: 'eac3', 布局: '5.1(side)', Channels: '6' }, true)])).toBe('EAC3 5.1')
  })
  it('does not invent missing parameters', () => {
    expect(mediaFileSummary('2026-01-01', [])).toBe('')
    expect(mediaFileSummary('512MB', [])).toBe('512 MB')
  })
})
