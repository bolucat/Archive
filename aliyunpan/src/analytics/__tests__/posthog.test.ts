import { describe, expect, it, vi } from 'vitest'

vi.mock('posthog-js', () => ({ default: { init: vi.fn(), identify: vi.fn(), capture: vi.fn() } }))
vi.mock('../../store', () => ({ useSettingStore: vi.fn(() => ({ uiLanguage: 'zh-CN' })) }))

import { redactAnalyticsSecrets, resolveCloudApiFailure, shouldCaptureCloudApiFailure } from '../posthog'

describe('analytics privacy contract', () => {
  it('uses UUID-shaped anonymous installation identifiers', () => {
    expect(crypto.randomUUID()).toMatch(/^[0-9a-f-]{36}$/)
  })

  it('keeps diagnostics while redacting credentials', () => {
    expect(resolveCloudApiFailure('https://api-drive.mypikpak.com/drive/v1/files?access_token=secret&parent_id=private', 403, { message: 'denied', refresh_token: 'secret' })).toEqual({ provider: 'pikpak', statusCode: 403, failureKind: 'http', requestUrl: 'https://api-drive.mypikpak.com/drive/v1/files?access_token=[REDACTED]&parent_id=private', serverError: '{"message":"denied","refresh_token":"[REDACTED]"}' })
    expect(resolveCloudApiFailure('https://pan.baidu.com/rest/2.0/xpan/file?access_token=secret', 401)).toMatchObject({ provider: 'baidu', requestUrl: 'https://pan.baidu.com/rest/2.0/xpan/file?access_token=[REDACTED]' })
    expect(resolveCloudApiFailure('https://example.com/private-file', 500)).toBeUndefined()
  })

  it('redacts credentials from network error text', () => {
    expect(redactAnalyticsSecrets('Authorization: Bearer abc\nCookie: sid=abc\npassword=abc')).toBe('Authorization:[REDACTED]\nCookie: [REDACTED]\npassword=[REDACTED]')
  })

  it('does not report Aliyun token refresh attempts as final failures', () => {
    const aliyun = resolveCloudApiFailure('https://api.aliyundrive.com/v2/file/download', 401)
    const baidu = resolveCloudApiFailure('https://pan.baidu.com/rest/2.0/xpan/file', 401)
    expect(aliyun && shouldCaptureCloudApiFailure(aliyun)).toBe(false)
    expect(baidu && shouldCaptureCloudApiFailure(baidu)).toBe(true)
  })
})
