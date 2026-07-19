import { afterEach, describe, expect, it, vi } from 'vitest'
import { requireMediaAcquisitionPro } from '../usageLimit'

function useStorage(values: Record<string, string> = {}) {
  const store = new Map(Object.entries(values))
  vi.stubGlobal('localStorage', {
    get length() { return store.size },
    key(index: number) { return [...store.keys()][index] || null },
    getItem(key: string) { return store.get(key) || null },
    setItem(key: string, value: string) { store.set(key, String(value)) },
    removeItem(key: string) { store.delete(key) }
  })
}

afterEach(() => vi.unstubAllGlobals())

describe('media acquisition Pro access', () => {
  it('rejects Agent media acquisition for non-Pro users', () => {
    useStorage({ app_user_authed: '1' })
    expect(() => requireMediaAcquisitionPro()).toThrow('Agent 获取资源和追更需购买 Pro 后使用')
  })

  it('allows Agent media acquisition for Pro users', () => {
    useStorage({ app_user_authed: '1', app_user_pro: '1' })
    expect(() => requireMediaAcquisitionPro()).not.toThrow()
  })
})
