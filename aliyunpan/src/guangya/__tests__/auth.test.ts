import { afterEach, describe, expect, it, vi } from 'vitest'
import { applyGuangyaQuota } from '../auth'

afterEach(() => vi.unstubAllGlobals())

describe('applyGuangyaQuota', () => {
  it('writes account capacity for the user popover', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ data: { username: 'gaozhangmin', total_size: 1000, used_size: 250 } })
    }))
    const token: any = { access_token: 'token', device_id: 'device', total_size: 0, used_size: 0, free_size: 0, spaceinfo: '' }

    await expect(applyGuangyaQuota(token)).resolves.toBe(true)
    expect(token).toMatchObject({ total_size: 1000, used_size: 250, free_size: 750, user_name: 'gaozhangmin' })
    expect(token.spaceinfo).toBeTruthy()
  })
})
