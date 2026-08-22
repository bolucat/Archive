import { describe, expect, it } from 'vitest'
import { buildMediaFingerprint } from '../mediaFingerprint'

describe('buildMediaFingerprint', () => {
  it('keeps the provider, declared hash algorithm, and file size together', () => {
    expect(buildMediaFingerprint({
      driveServerId: 'aliyun',
      driveId: 'drive-a',
      fileSize: 1024,
      contentHash: 'ABCDEF123456',
      contentHashName: 'sha1'
    })).toEqual({
      fingerprintNamespace: 'aliyun:sha1',
      fingerprint: 'abcdef123456',
      fileSize: 1024
    })
  })

  it('does not relabel an undeclared provider hash as MD5', () => {
    expect(buildMediaFingerprint({
      driveServerId: 'dropbox',
      driveId: 'dropbox',
      fileSize: 1024,
      contentHash: 'provider-specific-hash'
    })).toBeUndefined()
  })
})
