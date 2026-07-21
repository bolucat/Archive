import { describe, expect, it } from 'vitest'
import { captureVideoQualitySwitchPlaybackState } from '../videoQualitySwitch'

describe('video quality switching playback state', () => {
  it('preserves playing state and position for an active video', () => {
    expect(captureVideoQualitySwitchPlaybackState(true, 123.8)).toEqual({ shouldResume: true, position: 123.8 })
  })

  it('does not autoplay a video that was paused', () => {
    expect(captureVideoQualitySwitchPlaybackState(false, 0)).toEqual({ shouldResume: false, position: 0 })
  })

  it('normalizes invalid positions', () => {
    expect(captureVideoQualitySwitchPlaybackState(true, Number.NaN)).toEqual({ shouldResume: true, position: 0 })
  })
})
