import { describe, expect, it } from 'vitest'
import { bytesToSize, checkIsNeedRun, separateConfig } from '../../utils'

describe('bytesToSize', () => {
  it('formats bytes', () => {
    expect(bytesToSize(1024)).toBe('1.0 KB')
    expect(bytesToSize(1024 * 1024)).toBe('1.0 MB')
    expect(bytesToSize(0)).toBe('0 B')
  })
})

describe('checkIsNeedRun', () => {
  it('returns false when disabled', () => {
    expect(checkIsNeedRun(false, 0, 3600000)).toBe(false)
  })

  it('returns true when enabled and enough time has passed', () => {
    const lastTime = Date.now() - 3600001
    expect(checkIsNeedRun(true, lastTime, 3600000)).toBe(true)
  })

  it('returns false when not enough time has passed', () => {
    expect(checkIsNeedRun(true, Date.now(), 3600000)).toBe(false)
  })
})

describe('separateConfig', () => {
  it('splits config into system and user parts', () => {
    const { system, user } = separateConfig({ 'bt-tracker': 'x', 'auto-sync-tracker': true } as any)
    expect(system['bt-tracker']).toBe('x')
    expect(user['auto-sync-tracker']).toBe(true)
  })
})
