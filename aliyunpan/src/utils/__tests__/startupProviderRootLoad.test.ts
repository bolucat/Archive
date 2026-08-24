import { describe, expect, it } from 'vitest'
import { loadInitialProviderRoot, shouldRetryInitialRootLoad } from '../../user/startupProviderRootLoad'

describe('startup provider root loading', () => {
  it('retries a failed third-party initial root request once', async () => {
    let attempts = 0

    await loadInitialProviderRoot(true, async () => {
      attempts += 1
      if (attempts === 1) throw new Error('cold-start request failed')
    }, async () => undefined)

    expect(attempts).toBe(2)
    expect(shouldRetryInitialRootLoad('cloud123')).toBe(true)
    expect(shouldRetryInitialRootLoad('baidu')).toBe(true)
    expect(shouldRetryInitialRootLoad('139')).toBe(true)
  })

  it('does not retry the existing provider path', async () => {
    let attempts = 0

    await expect(loadInitialProviderRoot(false, async () => {
      attempts += 1
      throw new Error('request failed')
    }, async () => undefined)).rejects.toThrow('request failed')

    expect(attempts).toBe(1)
    expect(shouldRetryInitialRootLoad('aliyun')).toBe(false)
  })
})
