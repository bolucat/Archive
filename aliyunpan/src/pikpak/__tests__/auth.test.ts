import { describe, expect, it } from 'vitest'
import { buildPikPakCaptchaMeta } from '../auth'

describe('buildPikPakCaptchaMeta', () => {
  it('keeps a phone login in the username field used by sign-in', () => {
    expect(buildPikPakCaptchaMeta('15558182007')).toEqual({ username: '15558182007' })
  })

  it('keeps email and username logins unchanged', () => {
    expect(buildPikPakCaptchaMeta('user@example.com')).toEqual({ username: 'user@example.com' })
    expect(buildPikPakCaptchaMeta('pikpak-user')).toEqual({ username: 'pikpak-user' })
  })
})
