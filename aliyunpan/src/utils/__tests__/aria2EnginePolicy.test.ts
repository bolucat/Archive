import { describe, expect, it } from 'vitest'
import { normalizeAria2cEnabled, shouldUseNativeDownloadManager } from '../aria2EnginePolicy'

describe('aria2 engine policy', () => {
  it('keeps aria2c enabled even when old settings disabled it', () => {
    expect(normalizeAria2cEnabled(false)).toBe(true)
  })

  it('does not allow the native DownloadManager fallback', () => {
    expect(shouldUseNativeDownloadManager()).toBe(false)
  })
})
