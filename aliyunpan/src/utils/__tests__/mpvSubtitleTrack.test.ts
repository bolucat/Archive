import { describe, expect, it } from 'vitest'
import { getAutoSubtitleTrackId, subtitleSelectionCommands, bilingualSubtitlePositions } from '../mpvSubtitleTrack'

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

describe('subtitleSelectionCommands', () => {
  it('releases the primary slot before moving its track to secondary', () => {
    expect(subtitleSelectionCommands(true, 3, 3, -1)).toEqual([{ secondary: false, id: -1 }, { secondary: true, id: 3 }])
  })
  it('releases the secondary slot before moving its track to primary', () => {
    expect(subtitleSelectionCommands(false, 4, 3, 4)).toEqual([{ secondary: true, id: -1 }, { secondary: false, id: 4 }])
  })
  it('preserves a different primary track and restores it after sub-add selects a new track', () => {
    expect(subtitleSelectionCommands(true, 4, 3, -1)).toEqual([{ secondary: false, id: 3 }, { secondary: true, id: 4 }])
  })
  it('disables secondary subtitles without disabling the primary track', () => {
    expect(subtitleSelectionCommands(true, -1, 3, 4)).toEqual([{ secondary: false, id: 3 }, { secondary: true, id: -1 }])
  })
})

describe('bilingualSubtitlePositions', () => {
  it('stacks primary above secondary in the lower part of the frame', () => {
    const result = bilingualSubtitlePositions(96, 44, 1, true)
    expect(result.primary).toBeGreaterThan(80)
    expect(result.primary).toBeLessThan(result.secondary)
    expect(result.secondary).toBe(96)
  })
  it('keeps the chosen single-subtitle position when secondary is disabled', () => {
    expect(bilingualSubtitlePositions(96, 44, 1, false)).toEqual({ primary: 96, secondary: 96 })
  })
  it('reserves more room for larger fonts, zoom and multiline secondary text', () => {
    const normal = bilingualSubtitlePositions(96, 44, 1, true).primary
    expect(bilingualSubtitlePositions(96, 60, 1, true).primary).toBeLessThan(normal)
    expect(bilingualSubtitlePositions(96, 44, 2, true).primary).toBeLessThan(normal)
    expect(bilingualSubtitlePositions(96, 44, 1, true, 2).primary).toBeLessThan(normal)
  })
  it('moves both subtitles together and clamps the upper screen boundary', () => {
    const upper = bilingualSubtitlePositions(86, 44, 1, true)
    const lower = bilingualSubtitlePositions(96, 44, 1, true)
    expect(lower.primary - upper.primary).toBeCloseTo(10)
    expect(lower.secondary - upper.secondary).toBe(10)
    expect(bilingualSubtitlePositions(0, 80, 3, true, 4).primary).toBe(0)
  })
})
