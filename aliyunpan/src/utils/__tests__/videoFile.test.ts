import { describe, expect, it } from 'vitest'
import { isVideoFile } from '../videoFile'

describe('isVideoFile', () => {
  it('opens an MKV even when a cloud provider returns an unknown category', () => {
    expect(isVideoFile({ name: 'movie.MKV', ext: 'mkv', category: 'others' })).toBe(true)
  })

  it('keeps provider video categories playable', () => {
    expect(isVideoFile({ name: 'stream.bin', ext: 'bin', category: 'video2' })).toBe(true)
  })
})
