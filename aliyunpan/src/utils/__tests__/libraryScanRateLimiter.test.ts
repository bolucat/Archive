import { describe, expect, it } from 'vitest'
import { isScanRateLimitedError, libraryScanRateLimitScope } from '../libraryScanRateLimiter'

describe('library scan rate limiter', () => {
  it('uses one shared scope per cloud account and drive', () => {
    expect(libraryScanRateLimitScope('user-a', 'drive-a')).toBe('cloud:user-a')
    expect(libraryScanRateLimitScope('user-a', 'drive-b')).toBe('cloud:user-a')
  })

  it('recognizes provider and metadata API rate-limit responses', () => {
    expect(isScanRateLimitedError({ status: 429 })).toBe(true)
    expect(isScanRateLimitedError(new Error('429 Too Many Requests'))).toBe(true)
    expect(isScanRateLimitedError({ message: 'BlockException' })).toBe(true)
    expect(isScanRateLimitedError(new Error('network offline'))).toBe(false)
  })
})
