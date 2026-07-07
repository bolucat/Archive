import { describe, expect, it, vi } from 'vitest'

describe('bookManagerPreferences', () => {
  it('defines Koodo manager setting tabs while excluding account and sync', async () => {
    const { BOOK_MANAGER_SETTING_TABS } = await import('../bookManagerPreferences')

    expect(BOOK_MANAGER_SETTING_TABS.map((tab) => tab.key)).toEqual([
      'general',
      'data',
      'reading',
      'appearance',
      'more',
      'plugins',
      'ai',
      'background',
      'txt',
      'dict',
      'about',
    ])
    expect(BOOK_MANAGER_SETTING_TABS.map((tab) => tab.key)).not.toContain('account')
    expect(BOOK_MANAGER_SETTING_TABS.map((tab) => tab.key)).not.toContain('sync')
  })

  it('normalizes every supported Koodo homepage setting with defaults', async () => {
    const { DEFAULT_BOOK_MANAGER_PREFERENCES, normalizeBookManagerPreferences } = await import('../bookManagerPreferences')

    const prefs = normalizeBookManagerPreferences({
      isDisablePDFCover: true,
      isDisableCrop: true,
      isShowShelfBookCount: false,
      systemFont: 'Arial',
      themeColor: '#2f80ed',
      appSkin: 'night',
      customSystemCSS: '.manager { opacity: .9 }',
      startupShelf: 'Archive',
      defaultSearchEngine: 'duckduckgo',
      txtParsers: [{ label: 'Scene', regex: '^Scene\\s+\\d+' }],
      localDictionaries: [{ id: 'd1', name: 'Oxford', extension: 'mdx' }],
      backgroundImages: [{ id: 'b1', name: 'Paper', dataUrl: 'data:image/png;base64,abc' }],
    })

    expect(prefs.isDisablePDFCover).toBe(true)
    expect(prefs.isDisableCrop).toBe(true)
    expect(prefs.isShowShelfBookCount).toBe(false)
    expect(prefs.systemFont).toBe('Arial')
    expect(prefs.themeColor).toBe('#2f80ed')
    expect(prefs.appSkin).toBe('night')
    expect(prefs.customSystemCSS).toBe('.manager { opacity: .9 }')
    expect(prefs.startupShelf).toBe('Archive')
    expect(prefs.defaultSearchEngine).toBe('duckduckgo')
    expect(prefs.txtParsers).toEqual([{ label: 'Scene', regex: '^Scene\\s+\\d+' }])
    expect(prefs.localDictionaries).toEqual([{ id: 'd1', name: 'Oxford', extension: 'mdx' }])
    expect(prefs.backgroundImages).toEqual([{ id: 'b1', name: 'Paper', dataUrl: 'data:image/png;base64,abc' }])
    expect(Object.keys(DEFAULT_BOOK_MANAGER_PREFERENCES)).not.toContain('isEnableKoodoSync')
  })

  it('persists manager preferences through localStorage', async () => {
    const store = new Map<string, string>()
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => store.get(key) || null,
      setItem: (key: string, value: string) => store.set(key, value),
      removeItem: (key: string) => store.delete(key),
    })
    const { loadBookManagerPreferences, saveBookManagerPreferences } = await import('../bookManagerPreferences')

    saveBookManagerPreferences({ isHideShelfBook: true, themeColor: '#d94b8b' })

    expect(loadBookManagerPreferences().isHideShelfBook).toBe(true)
    expect(loadBookManagerPreferences().themeColor).toBe('#d94b8b')
    vi.unstubAllGlobals()
  })
})
