import { describe, expect, it, vi } from 'vitest'

vi.mock('posthog-js', () => ({ default: { init: vi.fn(), identify: vi.fn(), capture: vi.fn() } }))
vi.mock('../../store', () => ({ useSettingStore: vi.fn(() => ({ uiLanguage: 'zh-CN' })) }))

import { buildMediaScrapeUnrecognizedProperties, redactAnalyticsSecrets, resolveCloudApiFailure, shouldCaptureCloudApiFailure } from '../posthog'

describe('analytics privacy contract', () => {
  it('uses UUID-shaped anonymous installation identifiers', () => {
    expect(crypto.randomUUID()).toMatch(/^[0-9a-f-]{36}$/)
  })

  it('keeps diagnostics while redacting credentials', () => {
    expect(resolveCloudApiFailure('https://api-drive.mypikpak.com/drive/v1/files?access_token=secret&parent_id=private#fragment', 403, { message: 'denied', refresh_token: 'secret' })).toEqual({ provider: 'pikpak', statusCode: 403, failureKind: 'http', requestUrl: 'https://api-drive.mypikpak.com/drive/v1/files', serverError: '{"message":"denied","refresh_token":"[REDACTED]"}' })
    expect(resolveCloudApiFailure('https://pan.baidu.com/rest/2.0/xpan/file?access_token=secret', 401)).toMatchObject({ provider: 'baidu', requestUrl: 'https://pan.baidu.com/rest/2.0/xpan/file' })
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

  it('reports only filename-derived recognition signals for unmatched media', () => {
    expect(buildMediaScrapeUnrecognizedProperties({
      fileName: '/private/folder/Arrival.2016.2160p.mkv',
      normalizedFileName: 'arrival.2016.2160p',
      cleanedTitle: 'arrival',
      releaseYear: 2016,
      stage: 'tmdb_no_match',
      tmdbOutcome: 'no_match',
      aiOutcome: 'not_attempted',
      hasFingerprint: true,
      fingerprintAlgorithm: 'sha1'
    })).toEqual({
      file_name: 'Arrival.2016.2160p.mkv',
      file_extension: 'mkv',
      normalized_file_name: 'arrival.2016.2160p',
      cleaned_title: 'arrival',
      title_parse_status: 'valid_title',
      year_detected: true,
      episode_pattern_detected: false,
      normalizer_version: '2026-08-21.1',
      release_year: 2016,
      season_number: 0,
      episode_number: 0,
      failure_stage: 'tmdb_no_match',
      tmdb_outcome: 'no_match',
      ai_outcome: 'not_attempted',
      ai_candidate_title: '',
      ai_candidate_type: '',
      ai_candidate_year: 0,
      ai_candidate_season: 0,
      ai_candidate_episode: 0,
      ai_candidate_confidence: 0,
      has_content_fingerprint: true,
      fingerprint_algorithm: 'sha1',
      app_version: expect.any(String)
    })
  })

  it('keeps only structured AI candidates for recognition diagnostics', () => {
    const properties = buildMediaScrapeUnrecognizedProperties({
      fileName: 'Arrival.2016.2160p.mkv',
      normalizedFileName: 'arrival.2016.2160p',
      cleanedTitle: 'arrival',
      stage: 'ai_no_match',
      tmdbOutcome: 'no_match',
      aiOutcome: 'tmdb_no_match_after_ai',
      aiCandidate: { title: 'Arrival', type: 'movie', year: 2016, confidence: 0.876 },
      hasFingerprint: false
    })
    expect(properties).toMatchObject({
      ai_candidate_title: 'Arrival',
      ai_candidate_type: 'movie',
      ai_candidate_year: 2016,
      ai_candidate_season: 0,
      ai_candidate_episode: 0,
      ai_candidate_confidence: 0.88
    })
    expect(properties).not.toHaveProperty('ai_candidate_reason')
  })
})
