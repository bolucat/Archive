import { describe, expect, it } from 'vitest'
import { formatOssMultipartETag } from '../oss'

describe('formatOssMultipartETag', () => {
  it('preserves the quoted ETag required by CompleteMultipartUpload', () => {
    expect(formatOssMultipartETag('"abc123"')).toBe('"abc123"')
    expect(formatOssMultipartETag('abc123')).toBe('"abc123"')
  })
})
