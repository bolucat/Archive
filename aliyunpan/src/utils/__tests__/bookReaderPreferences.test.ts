import { describe, expect, it } from 'vitest'
import {
  DEFAULT_BOOK_READER_PREFERENCES,
  loadBookReaderPreferences,
  loadBookReaderStylePreferences,
  normalizeBookReaderPreferences,
  removeBookReaderStylePreferences,
  saveBookReaderStylePreferences,
  saveBookReaderPreferences
} from '../bookReaderPreferences'

function createStorage(seed: Record<string, string> = {}): Storage {
  const data = new Map(Object.entries(seed))
  return {
    get length() { return data.size },
    clear: () => data.clear(),
    getItem: (key: string) => data.get(key) ?? null,
    key: (index: number) => Array.from(data.keys())[index] ?? null,
    removeItem: (key: string) => { data.delete(key) },
    setItem: (key: string, value: string) => { data.set(key, value) }
  }
}

describe('bookReaderPreferences', () => {
  it('normalizes invalid preference values to defaults', () => {
    expect(normalizeBookReaderPreferences({
      themeMode: 'blue',
      fontSize: 100,
      readerLayoutMode: 'grid',
      readerIndent: 'yes',
      readerHyphenation: 'no',
      readerBionic: null,
      readerParaSpacing: '',
      readerLineHeight: '9',
      readerTextAlign: 'center',
      readerPageWidth: 2000,
      readerBackgroundColor: '',
      readerTextColor: '',
      readerVoiceLocale: 42,
      readerVoiceName: false,
      readerVoiceURI: null,
      readerVoiceRate: 9,
      readerPopupActionKeys: ['not-real'],
      readerTranslationTarget: ''
    })).toEqual({
      ...DEFAULT_BOOK_READER_PREFERENCES,
      fontSize: 40,
      readerMargin: 24,
      readerPageWidth: 1600,
      readerVoiceRate: 8
    })
  })

  it('loads preferences from storage', () => {
    const storage = createStorage({
      'bookReader.preferences': JSON.stringify({
        themeMode: 'dark',
        fontSize: 21,
        readerLayoutMode: 'scroll',
        readerIndent: false,
        readerHyphenation: true,
        readerBionic: true,
        readerParaSpacing: '48',
        readerLineHeight: '1.75',
        readerTextAlign: 'Justify',
        readerPageWidth: 1120,
        readerBackgroundColor: 'rgba(44,47,49,1)',
        readerTextColor: 'rgba(255,255,255,1)',
        readerVoiceLocale: 'zh-CN',
        readerVoiceName: 'Tingting',
        readerVoiceURI: 'voice-zh-cn-1',
        readerVoiceRate: 1.5,
        readerPopupActionKeys: ['note', 'highlight', 'translation', 'copy', 'speech-start'],
        readerTranslationTarget: 'en'
      })
    })

    expect(loadBookReaderPreferences(storage)).toEqual({
      ...DEFAULT_BOOK_READER_PREFERENCES,
      themeMode: 'dark',
      fontSize: 21,
      readerLayoutMode: 'scroll',
      readerIndent: false,
      readerHyphenation: true,
      readerBionic: true,
      readerParaSpacing: '48',
      readerLineHeight: '1.75',
      readerTextAlign: 'Justify',
      readerPageWidth: 1120,
      readerBackgroundColor: 'rgba(44,47,49,1)',
      readerTextColor: 'rgba(255,255,255,1)',
      readerVoiceLocale: 'zh-CN',
      readerVoiceName: 'Tingting',
      readerVoiceURI: 'voice-zh-cn-1',
      readerVoiceRate: 1.5,
      readerMargin: 24,
      readerPopupActionKeys: ['note', 'highlight', 'translation', 'copy', 'speech-start'],
      readerTranslationTarget: 'en'
    })
  })

  it('saves merged normalized preferences', () => {
    const storage = createStorage({
      'bookReader.preferences': JSON.stringify(DEFAULT_BOOK_READER_PREFERENCES)
    })

    const saved = saveBookReaderPreferences({ fontSize: 12, readerLayoutMode: 'double', readerTranslationTarget: 'en' }, storage)

    expect(saved).toEqual({
      ...DEFAULT_BOOK_READER_PREFERENCES,
      fontSize: 14,
      readerLayoutMode: 'double',
      readerTranslationTarget: 'en'
    })
    expect(loadBookReaderPreferences(storage)).toEqual(saved)
  })

  it('keeps independent reader styles scoped to one book', () => {
    const storage = createStorage()
    saveBookReaderStylePreferences('book-a', { ...DEFAULT_BOOK_READER_PREFERENCES, fontSize: 27, readerIsSeperateStyle: true }, storage)

    expect(loadBookReaderStylePreferences('book-a', storage)?.fontSize).toBe(27)
    expect(loadBookReaderStylePreferences('book-b', storage)).toBeNull()

    removeBookReaderStylePreferences('book-a', storage)
    expect(loadBookReaderStylePreferences('book-a', storage)).toBeNull()
  })
})
