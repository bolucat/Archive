import { describe, expect, it } from 'vitest'
import { reactive, isProxy } from 'vue'
import { mediaPersistenceSnapshot } from '../mediaPersistenceSnapshot'

describe('media persistence snapshot', () => {
  it('unwraps proxies nested in merged TV records and preserves dates/undefined', () => {
    const existing = reactive({ id: 'tv', genres: ['Drama'], credits: { cast: [{ name: 'Actor' }] }, seasons: [{ episodes: [{ driveFiles: [{ id: 'file' }] }] }], addedAt: new Date('2026-09-27'), year: undefined })
    const merged = { ...existing, seasons: [...existing.seasons] }
    expect(() => structuredClone(merged)).toThrow()
    const snapshot = mediaPersistenceSnapshot(merged)
    expect(isProxy(snapshot.genres)).toBe(false)
    expect(structuredClone(snapshot)).toEqual(snapshot)
    expect(snapshot.addedAt).toBeInstanceOf(Date)
    expect(snapshot.year).toBeUndefined()
    expect(snapshot.credits).not.toBe(existing.credits)
  })

  it('captures a stable batch before later reactive updates', () => {
    const item = reactive({ id: 'movie', driveFiles: [{ id: 'first' }] })
    const snapshot = mediaPersistenceSnapshot([item])
    item.driveFiles.push({ id: 'second' })
    expect(snapshot[0].driveFiles).toHaveLength(1)
  })
})
