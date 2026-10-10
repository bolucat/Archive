import { afterEach, describe, expect, it, vi } from 'vitest'
import { readPersonalRatings, savePersonalRating } from '../mediaPersonalRating'
describe('personal rating persistence', () => {
  afterEach(() => vi.unstubAllGlobals())
  it('persists independent ratings and restores them', () => {
    let value: string | null = null
    vi.stubGlobal('localStorage', { getItem: () => value, setItem: (_key: string, next: string) => { value = next } })
    savePersonalRating('one', 7); savePersonalRating('two', 10)
    expect(readPersonalRatings()).toEqual({ one: 7, two: 10 })
    expect(() => savePersonalRating('one', 11)).toThrow()
  })
  it('does not overwrite corrupt storage silently', () => {
    const setItem = vi.fn()
    vi.stubGlobal('localStorage', { getItem: () => '{broken', setItem })
    expect(() => savePersonalRating('one', 5)).toThrow()
    expect(setItem).not.toHaveBeenCalled()
  })
})
