import type { BookPopupActionKey } from './bookPopupActions'
import { buildPopupActions } from './bookPopupActions'

export type BookReaderThemeMode = 'paper' | 'eye' | 'dark'
export type BookReaderLayoutMode = 'single' | 'double' | 'scroll'
export type BookSelectAction = '' | 'translation' | 'dict' | 'highlight' | 'note' | 'speaker'
export type BookReaderBookLayout = '' | 'boxplayer' | 'heti' | 'han' | 'typo' | 'tufte' | 'typebase'
export type BookReaderConvertChinese = '' | 'Simplified To Traditional' | 'Traditional To Simplified'
export type BookReaderFullTranslationMode = 'no' | 'both' | 'target'

export interface BookReaderPreferences {
  themeMode: BookReaderThemeMode
  fontSize: number
  readerLayoutMode: BookReaderLayoutMode
  readerIndent: boolean
  readerHyphenation: boolean
  readerBionic: boolean
  readerParaSpacing: string
  readerLineHeight: string
  readerTextAlign: string
  readerPageWidth: number
  readerBackgroundColor: string
  readerTextColor: string
  readerVoiceLocale: string
  readerVoiceName: string
  readerVoiceURI: string
  readerVoiceRate: number
  readerPopupActionKeys: BookPopupActionKey[]
  readerFontFamily: string
  readerSubFontFamily: string
  readerMargin: number
  readerLetterSpacing: number
  readerScale: number
  readerBrightness: number
  readerSelectAction: BookSelectAction
  readerIsBold: boolean
  readerIsItalic: boolean
  readerIsUnderline: boolean
  readerIsShadow: boolean
  readerIsSliding: boolean
  readerIsOrphanWidow: boolean
  readerIsAllowScript: boolean
  readerIsAutoScroll: boolean
  readerBookLayout: BookReaderBookLayout
  readerConvertChinese: BookReaderConvertChinese
  readerFullTranslationMode: BookReaderFullTranslationMode
  readerTranslationTarget: string
  readerTextOrientation: string
  readerCustomCSS: string
  readerIsCustomCSS: boolean
  readerIsInvert: boolean
  readerIsStartFromEven: boolean
  readerIsShowPageBorder: boolean
  readerIsHideFooter: boolean
  readerIsHideHeader: boolean
  readerIsHideBackground: boolean
  readerIsHidePageButton: boolean
  readerIsHideMenuButton: boolean
  readerIsHideAudiobookButton: boolean
  readerIsHideAIButton: boolean
  readerIsHideScaleButton: boolean
  readerIsHidePDFConvertButton: boolean
  readerIsSeperateStyle: boolean
  readerIsWordDefinition: boolean
}

const LS_KEY = 'bookReader.preferences'
export const READER_BACKGROUND_COLORS = ['rgba(255,255,255,1)', 'rgba(44,47,49,1)', 'rgba(233, 216, 188,1)', 'rgba(197, 231, 207,1)']
export const READER_TEXT_COLORS = ['rgba(0,0,0,1)', 'rgba(255,255,255,1)', 'rgba(89, 68, 41,1)', 'rgba(54, 80, 62,1)']

export const DEFAULT_BOOK_READER_PREFERENCES: BookReaderPreferences = {
  themeMode: 'eye',
  fontSize: 18,
  readerLayoutMode: 'single',
  readerIndent: true,
  readerHyphenation: false,
  readerBionic: false,
  readerParaSpacing: '24',
  readerLineHeight: '1.5',
  readerTextAlign: '',
  readerPageWidth: 980,
  readerBackgroundColor: READER_BACKGROUND_COLORS[0],
  readerTextColor: READER_TEXT_COLORS[0],
  readerVoiceLocale: '',
  readerVoiceName: '',
  readerVoiceURI: '',
  readerVoiceRate: 1,
  readerPopupActionKeys: buildPopupActions().map((item) => item.key),
  readerFontFamily: 'Built-in font',
  readerSubFontFamily: '',
  readerMargin: 24,
  readerLetterSpacing: 0,
  readerScale: 1,
  readerBrightness: 1,
  readerSelectAction: '',
  readerIsBold: false,
  readerIsItalic: false,
  readerIsUnderline: false,
  readerIsShadow: false,
  readerIsSliding: false,
  readerIsOrphanWidow: true,
  readerIsAllowScript: false,
  readerIsAutoScroll: false,
  readerBookLayout: '',
  readerConvertChinese: '',
  readerFullTranslationMode: 'no',
  readerTranslationTarget: 'zh',
  readerTextOrientation: '',
  readerCustomCSS: '',
  readerIsCustomCSS: false,
  readerIsInvert: false,
  readerIsStartFromEven: false,
  readerIsShowPageBorder: false,
  readerIsHideFooter: false,
  readerIsHideHeader: false,
  readerIsHideBackground: false,
  readerIsHidePageButton: false,
  readerIsHideMenuButton: false,
  readerIsHideAudiobookButton: false,
  readerIsHideAIButton: false,
  readerIsHideScaleButton: false,
  readerIsHidePDFConvertButton: false,
  readerIsSeperateStyle: false,
  readerIsWordDefinition: false
}

function isThemeMode(value: unknown): value is BookReaderThemeMode {
  return value === 'paper' || value === 'eye' || value === 'dark'
}

function isReaderLayoutMode(value: unknown): value is BookReaderLayoutMode {
  return value === 'single' || value === 'double' || value === 'scroll'
}

function isReaderParaSpacing(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}

function isReaderLineHeight(value: unknown): value is string {
  return value === '' || value === '1' || value === '1.25' || value === '1.5' || value === '1.75' || value === '2'
}

function isReaderTextAlign(value: unknown): value is string {
  return value === '' || value === 'Left' || value === 'Justify' || value === 'Right'
}

function isReaderBackgroundColor(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}

function isReaderTextColor(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}

function isBookSelectAction(value: unknown): value is BookSelectAction {
  return value === '' || value === 'translation' || value === 'dict' || value === 'highlight' || value === 'note' || value === 'speaker'
}

function isBookReaderBookLayout(value: unknown): value is BookReaderBookLayout {
  return value === '' || value === 'boxplayer' || value === 'heti' || value === 'han' || value === 'typo' || value === 'tufte' || value === 'typebase'
}

function isBookReaderConvertChinese(value: unknown): value is BookReaderConvertChinese {
  return value === '' || value === 'Simplified To Traditional' || value === 'Traditional To Simplified'
}

function isBookReaderFullTranslationMode(value: unknown): value is BookReaderFullTranslationMode {
  return value === 'no' || value === 'both' || value === 'target'
}

function normalizeTranslationTarget(value: unknown): string {
  return typeof value === 'string' && value.trim() ? value.trim() : DEFAULT_BOOK_READER_PREFERENCES.readerTranslationTarget
}

function clampNumber(value: unknown, min: number, max: number, fallback: number): number {
  const n = Number(value)
  return Number.isFinite(n) ? Math.max(min, Math.min(max, n)) : fallback
}

function normalizeBookPopupActionKeys(value: unknown): BookPopupActionKey[] {
  if (!Array.isArray(value)) return DEFAULT_BOOK_READER_PREFERENCES.readerPopupActionKeys
  const normalized = buildPopupActions(value as BookPopupActionKey[]).map((item) => item.key)
  return normalized.length ? normalized : DEFAULT_BOOK_READER_PREFERENCES.readerPopupActionKeys
}

export function normalizeBookReaderPreferences(value: unknown): BookReaderPreferences {
  const raw = value && typeof value === 'object' ? (value as Partial<BookReaderPreferences>) : {}
  const fontSize = Number(raw.fontSize)
  const readerPageWidth = Number(raw.readerPageWidth)
  const readerVoiceRate = Number(raw.readerVoiceRate)
  return {
    themeMode: isThemeMode(raw.themeMode) ? raw.themeMode : DEFAULT_BOOK_READER_PREFERENCES.themeMode,
    fontSize: Number.isFinite(fontSize) ? Math.max(14, Math.min(40, Math.round(fontSize))) : DEFAULT_BOOK_READER_PREFERENCES.fontSize,
    readerLayoutMode: isReaderLayoutMode(raw.readerLayoutMode) ? raw.readerLayoutMode : DEFAULT_BOOK_READER_PREFERENCES.readerLayoutMode,
    readerIndent: typeof raw.readerIndent === 'boolean' ? raw.readerIndent : DEFAULT_BOOK_READER_PREFERENCES.readerIndent,
    readerHyphenation: typeof raw.readerHyphenation === 'boolean' ? raw.readerHyphenation : DEFAULT_BOOK_READER_PREFERENCES.readerHyphenation,
    readerBionic: typeof raw.readerBionic === 'boolean' ? raw.readerBionic : DEFAULT_BOOK_READER_PREFERENCES.readerBionic,
    readerParaSpacing: isReaderParaSpacing(raw.readerParaSpacing) ? raw.readerParaSpacing : DEFAULT_BOOK_READER_PREFERENCES.readerParaSpacing,
    readerLineHeight: isReaderLineHeight(raw.readerLineHeight) ? raw.readerLineHeight : DEFAULT_BOOK_READER_PREFERENCES.readerLineHeight,
    readerTextAlign: isReaderTextAlign(raw.readerTextAlign) ? raw.readerTextAlign : DEFAULT_BOOK_READER_PREFERENCES.readerTextAlign,
    readerPageWidth: Number.isFinite(readerPageWidth) ? Math.max(720, Math.min(1600, Math.round(readerPageWidth))) : DEFAULT_BOOK_READER_PREFERENCES.readerPageWidth,
    readerBackgroundColor: isReaderBackgroundColor(raw.readerBackgroundColor) ? raw.readerBackgroundColor : DEFAULT_BOOK_READER_PREFERENCES.readerBackgroundColor,
    readerTextColor: isReaderTextColor(raw.readerTextColor) ? raw.readerTextColor : DEFAULT_BOOK_READER_PREFERENCES.readerTextColor,
    readerVoiceLocale: typeof raw.readerVoiceLocale === 'string' ? raw.readerVoiceLocale : '',
    readerVoiceName: typeof raw.readerVoiceName === 'string' ? raw.readerVoiceName : '',
    readerVoiceURI: typeof raw.readerVoiceURI === 'string' ? raw.readerVoiceURI : '',
    readerVoiceRate: Number.isFinite(readerVoiceRate) ? Math.max(0.5, Math.min(8, Math.round(readerVoiceRate * 10) / 10)) : DEFAULT_BOOK_READER_PREFERENCES.readerVoiceRate,
    readerPopupActionKeys: normalizeBookPopupActionKeys(raw.readerPopupActionKeys),
    readerFontFamily: typeof raw.readerFontFamily === 'string' ? raw.readerFontFamily : DEFAULT_BOOK_READER_PREFERENCES.readerFontFamily,
    readerSubFontFamily: typeof raw.readerSubFontFamily === 'string' ? raw.readerSubFontFamily : '',
    readerMargin: clampNumber(raw.readerMargin, -40, 80, 0),
    readerLetterSpacing: clampNumber(raw.readerLetterSpacing, 0, 20, 0),
    readerScale: clampNumber(raw.readerScale, 0.5, 3, 1),
    readerBrightness: clampNumber(raw.readerBrightness, 0.3, 1, 1),
    readerSelectAction: isBookSelectAction(raw.readerSelectAction) ? raw.readerSelectAction : '',
    readerIsBold: typeof raw.readerIsBold === 'boolean' ? raw.readerIsBold : false,
    readerIsItalic: typeof raw.readerIsItalic === 'boolean' ? raw.readerIsItalic : false,
    readerIsUnderline: typeof raw.readerIsUnderline === 'boolean' ? raw.readerIsUnderline : false,
    readerIsShadow: typeof raw.readerIsShadow === 'boolean' ? raw.readerIsShadow : false,
    readerIsSliding: typeof raw.readerIsSliding === 'boolean' ? raw.readerIsSliding : false,
    readerIsOrphanWidow: typeof raw.readerIsOrphanWidow === 'boolean' ? raw.readerIsOrphanWidow : true,
    readerIsAllowScript: typeof raw.readerIsAllowScript === 'boolean' ? raw.readerIsAllowScript : false,
    readerIsAutoScroll: typeof raw.readerIsAutoScroll === 'boolean' ? raw.readerIsAutoScroll : false,
    readerBookLayout: isBookReaderBookLayout(raw.readerBookLayout) ? raw.readerBookLayout : '',
    readerConvertChinese: isBookReaderConvertChinese(raw.readerConvertChinese) ? raw.readerConvertChinese : '',
    readerFullTranslationMode: isBookReaderFullTranslationMode(raw.readerFullTranslationMode) ? raw.readerFullTranslationMode : 'no',
    readerTranslationTarget: normalizeTranslationTarget(raw.readerTranslationTarget),
    readerTextOrientation: typeof raw.readerTextOrientation === 'string' ? raw.readerTextOrientation : '',
    readerCustomCSS: typeof raw.readerCustomCSS === 'string' ? raw.readerCustomCSS : '',
    readerIsCustomCSS: typeof raw.readerIsCustomCSS === 'boolean' ? raw.readerIsCustomCSS : false,
    readerIsInvert: typeof raw.readerIsInvert === 'boolean' ? raw.readerIsInvert : false,
    readerIsStartFromEven: typeof raw.readerIsStartFromEven === 'boolean' ? raw.readerIsStartFromEven : false,
    readerIsShowPageBorder: typeof raw.readerIsShowPageBorder === 'boolean' ? raw.readerIsShowPageBorder : false,
    readerIsHideFooter: typeof raw.readerIsHideFooter === 'boolean' ? raw.readerIsHideFooter : false,
    readerIsHideHeader: typeof raw.readerIsHideHeader === 'boolean' ? raw.readerIsHideHeader : false,
    readerIsHideBackground: typeof raw.readerIsHideBackground === 'boolean' ? raw.readerIsHideBackground : false,
    readerIsHidePageButton: typeof raw.readerIsHidePageButton === 'boolean' ? raw.readerIsHidePageButton : false,
    readerIsHideMenuButton: typeof raw.readerIsHideMenuButton === 'boolean' ? raw.readerIsHideMenuButton : false,
    readerIsHideAudiobookButton: typeof raw.readerIsHideAudiobookButton === 'boolean' ? raw.readerIsHideAudiobookButton : false,
    readerIsHideAIButton: typeof raw.readerIsHideAIButton === 'boolean' ? raw.readerIsHideAIButton : false,
    readerIsHideScaleButton: typeof raw.readerIsHideScaleButton === 'boolean' ? raw.readerIsHideScaleButton : false,
    readerIsHidePDFConvertButton: typeof raw.readerIsHidePDFConvertButton === 'boolean' ? raw.readerIsHidePDFConvertButton : false,
    readerIsSeperateStyle: typeof raw.readerIsSeperateStyle === 'boolean' ? raw.readerIsSeperateStyle : false,
    readerIsWordDefinition: typeof raw.readerIsWordDefinition === 'boolean' ? raw.readerIsWordDefinition : false
  }
}

export function loadBookReaderPreferences(storage: Storage = localStorage): BookReaderPreferences {
  try {
    const raw = storage.getItem(LS_KEY)
    return normalizeBookReaderPreferences(raw ? JSON.parse(raw) : undefined)
  } catch {
    return { ...DEFAULT_BOOK_READER_PREFERENCES }
  }
}

export function saveBookReaderPreferences(preferences: Partial<BookReaderPreferences>, storage: Storage = localStorage): BookReaderPreferences {
  const normalized = normalizeBookReaderPreferences({
    ...loadBookReaderPreferences(storage),
    ...preferences
  })
  try {
    storage.setItem(LS_KEY, JSON.stringify(normalized))
  } catch {}
  return normalized
}
