import { afterEach, describe, expect, it } from 'vitest'
import { getLocalVideoProgress, saveLocalVideoProgress, videoProgressStorageKey } from '../videoProgress'

const values = new Map<string, string>()
const storage = {
  getItem: (key: string) => values.get(key) || null,
  setItem: (key: string, value: string) => values.set(key, value)
}

describe('local video progress', () => {
  afterEach(() => {
    values.clear()
    delete (globalThis as any).window
  })

  it('saves and reads progress by account, drive, and file', () => {
    Object.defineProperty(globalThis, 'window', { configurable: true, value: { localStorage: storage } })
    saveLocalVideoProgress('user-1', 'drive115', 'file-1', 123.9)
    expect(getLocalVideoProgress('user-1', 'drive115', 'file-1')).toBe(123)
    expect(values.has(videoProgressStorageKey('user-1', 'drive115', 'file-1'))).toBe(true)
  })

  it('ignores invalid progress values', () => {
    Object.defineProperty(globalThis, 'window', { configurable: true, value: { localStorage: storage } })
    saveLocalVideoProgress('user-1', 'drive115', 'file-1', 0)
    expect(getLocalVideoProgress('user-1', 'drive115', 'file-1')).toBe(0)
  })

  it('normalizes the 115 drive alias to the canonical drive id', () => {
    Object.defineProperty(globalThis, 'window', { configurable: true, value: { localStorage: storage } })
    saveLocalVideoProgress('user-1', '115', 'file-1', 456)
    expect(getLocalVideoProgress('user-1', 'drive115', 'file-1')).toBe(456)
    expect(values.has(videoProgressStorageKey('user-1', 'drive115', 'file-1'))).toBe(true)
  })
})
