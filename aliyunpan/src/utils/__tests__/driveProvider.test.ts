import { describe, expect, it } from 'vitest'
import { isProviderTokenForUser, isTokenCompatibleWithDrive, resolveDriveProvider } from '../driveProvider'

describe('resolveDriveProvider', () => {
  it('uses a known third-party drive id when the account cache is unavailable', () => {
    expect(resolveDriveProvider('missing-user', 'box')).toMatchObject({ provider: 'box', isValid: true })
  })

  it('rejects mismatched account and drive providers instead of falling through', () => {
    expect(resolveDriveProvider('baidu_user', 'box')).toMatchObject({ provider: 'unknown', isValid: false })
  })

  it('allows provider-specific dynamic drive ids when the account is known', () => {
    expect(resolveDriveProvider('cloud123_user', 'actual-cloud123-drive')).toMatchObject({ provider: 'cloud123', isValid: true })
  })

  it('does not classify an unknown account and drive as Aliyun', () => {
    expect(resolveDriveProvider('missing-user', 'unrecognized-drive')).toMatchObject({ provider: 'unknown', isValid: false })
  })

  it('rejects a file owner token that belongs to a different provider', () => {
    expect(isTokenCompatibleWithDrive({ user_id: 'aliyun_user', tokenfrom: 'aliyun' }, 'google')).toBe(false)
    expect(isTokenCompatibleWithDrive({ user_id: 'google_user', tokenfrom: 'google' }, 'google')).toBe(true)
  })

  it('requires the requested account and provider before using a token', () => {
    const token = { user_id: 'same-user', tokenfrom: 'aliyun', access_token: 'token' } as any
    expect(isProviderTokenForUser(token, 'same-user', '115')).toBe(false)
    expect(isProviderTokenForUser(token, 'other-user', 'aliyun')).toBe(false)
    expect(isProviderTokenForUser(token, 'same-user', 'aliyun')).toBe(true)
  })
})
