import { describe, expect, it } from 'vitest'
import { getCloudDownloadSourceLabel, getCloudProviderLabel } from '../../down/cloudDownloadSource'

describe('cloud download source labels', () => {
  it('labels the provider from offlineProvider and includes the account display name', () => {
    expect(getCloudProviderLabel({ offlineProvider: 'drive115', drive_id: 'drive115' })).toBe('115 网盘')
    expect(getCloudDownloadSourceLabel({ offlineProvider: 'drive115', drive_id: 'drive115', user_id: '115_u1' }, '我的 115')).toBe('115 网盘 · 我的 115')
  })

  it('falls back to the task user id when no account display name is available', () => {
    expect(getCloudDownloadSourceLabel({ offlineProvider: 'cloud123', drive_id: 'cloud123', user_id: 'cloud123_u1' })).toBe('123 云盘 · cloud123_u1')
  })
})
