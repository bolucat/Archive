import { describe, expect, it } from 'vitest'
import { normalizeDrive115OssCallback, normalizeDrive115UploadTokens } from '../../cloud115/uploadtoken'

describe('115 upload token normalization', () => {
  it('accepts the official single credential object', () => {
    expect(normalizeDrive115UploadTokens({ endpoint: 'https://oss.example.com', AccessKeyId: 'key-id', AccessKeySecret: 'key-secret', SecurityToken: 'security-token' })).toEqual([
      { endpoint: 'https://oss.example.com', AccessKeyId: 'key-id', AccessKeySecret: 'key-secret', SecurityToken: 'security-token' }
    ])
  })

  it('keeps compatibility with array responses and the historical secret spelling', () => {
    expect(normalizeDrive115UploadTokens([{ AccessKeyId: 'key-id', AccessKeySecrett: 'key-secret' }])).toEqual([{ AccessKeyId: 'key-id', AccessKeySecrett: 'key-secret', AccessKeySecret: 'key-secret' }])
  })

  it('unwraps the callback object returned by upload initialization', () => {
    expect(normalizeDrive115OssCallback({ callback: 'callback-value', callback_var: 'callback-var' })).toEqual({ callback: 'callback-value', callback_var: 'callback-var' })
    expect(normalizeDrive115OssCallback([{ callback: 'callback-value' }], 'callback-var')).toEqual({ callback: 'callback-value', callback_var: 'callback-var' })
  })
})
