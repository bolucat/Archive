import { afterEach, describe, expect, it, vi } from 'vitest'

afterEach(() => { vi.unstubAllGlobals(); vi.resetModules() })

describe('persistent library layout', () => {
  it('shares the selection and restores it after reloading', async () => {
    const values = new Map<string, string>()
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value)
    })
    const first = await import('../../store/libraryBrowseMode')
    expect(first.useLibraryBrowseMode().value).toBe('grid')
    first.useLibraryBrowseMode().value = 'list'
    expect(first.useLibraryBrowseMode().value).toBe('list')
    vi.resetModules()
    const restored = await import('../../store/libraryBrowseMode')
    expect(restored.useLibraryBrowseMode().value).toBe('list')
    restored.useLibraryBrowseMode().value = 'grid'
    expect(values.get(restored.libraryBrowseModeKey)).toBe('grid')
  })
  it('falls back safely for unavailable or invalid storage', async () => {
    const { readLibraryBrowseMode } = await import('../../store/libraryBrowseMode')
    expect(readLibraryBrowseMode({ getItem: () => 'invalid' })).toBe('grid')
    expect(readLibraryBrowseMode({ getItem: () => { throw new Error('unavailable') } })).toBe('grid')
  })
})
