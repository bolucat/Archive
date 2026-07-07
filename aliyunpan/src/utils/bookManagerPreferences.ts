export type BookManagerSettingTabKey =
  | 'general'
  | 'data'
  | 'reading'
  | 'appearance'
  | 'more'
  | 'plugins'
  | 'ai'
  | 'background'
  | 'txt'
  | 'dict'
  | 'about'

export type BookManagerSkin = 'system' | 'light' | 'night'

export interface BookManagerSettingTab {
  key: BookManagerSettingTabKey
  label: string
}

export interface BookManagerTxtParser {
  label: string
  regex: string
}

export interface BookManagerLocalDictionary {
  id: string
  name: string
  extension: string
}

export interface BookManagerBackgroundImage {
  id: string
  name: string
  dataUrl: string
}

export interface BookManagerPreferences {
  isImportPath: boolean
  isDisableTrashBin: boolean
  isHideShelfBook: boolean
  isDeleteShelfBook: boolean
  isPreventSleep: boolean
  isAlwaysOnTop: boolean
  isAutoMaximizeWin: boolean
  isAutoLaunch: boolean
  isMinimizeToTray: boolean
  isPreventAdd: boolean
  isPrecacheBook: boolean
  isExportOriginalName: boolean
  isDisableAI: boolean
  isUseOriginalName: boolean
  isDisableUpdate: boolean
  isDeleteOriginal: boolean
  isUseBuiltIn: boolean
  isEnableDiscordRPC: boolean
  isTouch: boolean
  isPreventTrigger: boolean
  isMergeWord: boolean
  isOpenInMain: boolean
  isManualScroll: boolean
  isOpenBook: boolean
  isAutoMaximize: boolean
  isAutoFullscreen: boolean
  isDisablePopup: boolean
  isDisableAutoScroll: boolean
  isOverwriteLink: boolean
  isOverwriteText: boolean
  isDisablePDFCover: boolean
  isDisableCrop: boolean
  isShowShelfBookCount: boolean
  systemFont: string
  themeColor: string
  appSkin: BookManagerSkin
  isCustomSystemCSS: boolean
  customSystemCSS: string
  startupShelf: string
  defaultSearchEngine: string
  txtParsers: BookManagerTxtParser[]
  localDictionaries: BookManagerLocalDictionary[]
  backgroundImages: BookManagerBackgroundImage[]
  appBackgroundImageId: string
  readerBackgroundImageId: string
}

export const BOOK_MANAGER_PREFERENCES_KEY = 'bookLibrary.koodoManagerPreferences'

export const BOOK_MANAGER_SETTING_TABS: BookManagerSettingTab[] = [
  { key: 'general', label: '通用' },
  { key: 'data', label: '数据' },
  { key: 'reading', label: '阅读' },
  { key: 'appearance', label: '外观' },
  { key: 'more', label: '更多设置' },
  { key: 'about', label: '关于' },
]

// 第二组（分隔线下方）
export const BOOK_MANAGER_SETTING_TABS_GROUP2: BookManagerSettingTab[] = [
  { key: 'plugins', label: '插件' },
  { key: 'ai', label: 'AI 服务' },
  { key: 'txt', label: 'TXT 解析器' },
  { key: 'dict', label: '本地词典' },
]

export const BOOK_MANAGER_THEME_COLORS = [
  { label: '默认', value: 'default', color: '#4b4b4b' },
  { label: '碧落', value: '#2f80d1', color: '#2f80d1' },
  { label: '苍翠', value: '#399393', color: '#399393' },
  { label: '胭脂', value: '#df6763', color: '#df6763' },
  { label: '紫苑', value: '#6861c9', color: '#6861c9' },
  { label: '橘黄', value: '#ec7d2c', color: '#ec7d2c' },
  { label: '芙蓉', value: '#d94b8b', color: '#d94b8b' },
  { label: '流金', value: '#e3b432', color: '#e3b432' },
  { label: '幽兰', value: '#7a58e6', color: '#7a58e6' },
  { label: '霁蓝', value: '#459bd8', color: '#459bd8' },
  { label: '烟岚', value: '#66758a', color: '#66758a' },
]

export const DEFAULT_TXT_PARSERS: BookManagerTxtParser[] = [
  { label: 'Chapter', regex: '^(Chapter|Part|Book|CHAPTER|PART|BOOK)\\b.*$' },
  { label: '中文章节', regex: '^第[一二三四五六七八九十百千万0-9]+[章节卷部].*$' },
]

export const DEFAULT_BOOK_MANAGER_PREFERENCES: BookManagerPreferences = {
  isImportPath: false,
  isDisableTrashBin: false,
  isHideShelfBook: false,
  isDeleteShelfBook: false,
  isPreventSleep: false,
  isAlwaysOnTop: false,
  isAutoMaximizeWin: false,
  isAutoLaunch: false,
  isMinimizeToTray: false,
  isPreventAdd: false,
  isPrecacheBook: false,
  isExportOriginalName: false,
  isDisableAI: false,
  isUseOriginalName: false,
  isDisableUpdate: false,
  isDeleteOriginal: false,
  isUseBuiltIn: false,
  isEnableDiscordRPC: false,
  isTouch: false,
  isPreventTrigger: false,
  isMergeWord: false,
  isOpenInMain: true,
  isManualScroll: false,
  isOpenBook: false,
  isAutoMaximize: false,
  isAutoFullscreen: false,
  isDisablePopup: false,
  isDisableAutoScroll: false,
  isOverwriteLink: false,
  isOverwriteText: false,
  isDisablePDFCover: false,
  isDisableCrop: false,
  isShowShelfBookCount: true,
  systemFont: 'Built-in font',
  themeColor: 'default',
  appSkin: 'system',
  isCustomSystemCSS: false,
  customSystemCSS: '',
  startupShelf: '',
  defaultSearchEngine: 'google',
  txtParsers: DEFAULT_TXT_PARSERS,
  localDictionaries: [],
  backgroundImages: [],
  appBackgroundImageId: '',
  readerBackgroundImageId: '',
}

function bool(value: unknown, fallback: boolean): boolean {
  return typeof value === 'boolean' ? value : fallback
}

function text(value: unknown, fallback: string): string {
  return typeof value === 'string' ? value : fallback
}

function skin(value: unknown): BookManagerSkin {
  return value === 'light' || value === 'night' || value === 'system' ? value : DEFAULT_BOOK_MANAGER_PREFERENCES.appSkin
}

function txtParsers(value: unknown): BookManagerTxtParser[] {
  if (!Array.isArray(value)) return [...DEFAULT_TXT_PARSERS]
  const normalized = value
    .map((item) => ({
      label: text(item?.label, '').trim(),
      regex: text(item?.regex, '').trim(),
    }))
    .filter((item) => item.label && item.regex)
  return normalized.length ? normalized : [...DEFAULT_TXT_PARSERS]
}

function dictionaries(value: unknown): BookManagerLocalDictionary[] {
  if (!Array.isArray(value)) return []
  return value
    .map((item) => ({
      id: text(item?.id, '').trim(),
      name: text(item?.name, '').trim(),
      extension: text(item?.extension, 'mdx').trim().toLowerCase() || 'mdx',
    }))
    .filter((item) => item.id && item.name)
}

function backgrounds(value: unknown): BookManagerBackgroundImage[] {
  if (!Array.isArray(value)) return []
  return value
    .map((item) => ({
      id: text(item?.id, '').trim(),
      name: text(item?.name, '').trim(),
      dataUrl: text(item?.dataUrl, '').trim(),
    }))
    .filter((item) => item.id && item.name && item.dataUrl.startsWith('data:image/'))
}

export function normalizeBookManagerPreferences(raw: Partial<BookManagerPreferences> = {}): BookManagerPreferences {
  const defaults = DEFAULT_BOOK_MANAGER_PREFERENCES
  return {
    ...defaults,
    isImportPath: bool(raw.isImportPath, defaults.isImportPath),
    isDisableTrashBin: bool(raw.isDisableTrashBin, defaults.isDisableTrashBin),
    isHideShelfBook: bool(raw.isHideShelfBook, defaults.isHideShelfBook),
    isDeleteShelfBook: bool(raw.isDeleteShelfBook, defaults.isDeleteShelfBook),
    isPreventSleep: bool(raw.isPreventSleep, defaults.isPreventSleep),
    isAlwaysOnTop: bool(raw.isAlwaysOnTop, defaults.isAlwaysOnTop),
    isAutoMaximizeWin: bool(raw.isAutoMaximizeWin, defaults.isAutoMaximizeWin),
    isAutoLaunch: bool(raw.isAutoLaunch, defaults.isAutoLaunch),
    isMinimizeToTray: bool(raw.isMinimizeToTray, defaults.isMinimizeToTray),
    isPreventAdd: bool(raw.isPreventAdd, defaults.isPreventAdd),
    isPrecacheBook: bool(raw.isPrecacheBook, defaults.isPrecacheBook),
    isExportOriginalName: bool(raw.isExportOriginalName, defaults.isExportOriginalName),
    isDisableAI: bool(raw.isDisableAI, defaults.isDisableAI),
    isUseOriginalName: bool(raw.isUseOriginalName, defaults.isUseOriginalName),
    isDisableUpdate: bool(raw.isDisableUpdate, defaults.isDisableUpdate),
    isDeleteOriginal: bool(raw.isDeleteOriginal, defaults.isDeleteOriginal),
    isUseBuiltIn: bool(raw.isUseBuiltIn, defaults.isUseBuiltIn),
    isEnableDiscordRPC: bool(raw.isEnableDiscordRPC, defaults.isEnableDiscordRPC),
    isTouch: bool(raw.isTouch, defaults.isTouch),
    isPreventTrigger: bool(raw.isPreventTrigger, defaults.isPreventTrigger),
    isMergeWord: bool(raw.isMergeWord, defaults.isMergeWord),
    isOpenInMain: bool(raw.isOpenInMain, defaults.isOpenInMain),
    isManualScroll: bool(raw.isManualScroll, defaults.isManualScroll),
    isOpenBook: bool(raw.isOpenBook, defaults.isOpenBook),
    isAutoMaximize: bool(raw.isAutoMaximize, defaults.isAutoMaximize),
    isAutoFullscreen: bool(raw.isAutoFullscreen, defaults.isAutoFullscreen),
    isDisablePopup: bool(raw.isDisablePopup, defaults.isDisablePopup),
    isDisableAutoScroll: bool(raw.isDisableAutoScroll, defaults.isDisableAutoScroll),
    isOverwriteLink: bool(raw.isOverwriteLink, defaults.isOverwriteLink),
    isOverwriteText: bool(raw.isOverwriteText, defaults.isOverwriteText),
    isDisablePDFCover: bool(raw.isDisablePDFCover, defaults.isDisablePDFCover),
    isDisableCrop: bool(raw.isDisableCrop, defaults.isDisableCrop),
    isShowShelfBookCount: bool(raw.isShowShelfBookCount, defaults.isShowShelfBookCount),
    systemFont: text(raw.systemFont, defaults.systemFont),
    themeColor: text(raw.themeColor, defaults.themeColor),
    appSkin: skin(raw.appSkin),
    isCustomSystemCSS: bool(raw.isCustomSystemCSS, defaults.isCustomSystemCSS),
    customSystemCSS: text(raw.customSystemCSS, defaults.customSystemCSS),
    startupShelf: text(raw.startupShelf, defaults.startupShelf),
    defaultSearchEngine: text(raw.defaultSearchEngine, defaults.defaultSearchEngine),
    txtParsers: txtParsers(raw.txtParsers),
    localDictionaries: dictionaries(raw.localDictionaries),
    backgroundImages: backgrounds(raw.backgroundImages),
    appBackgroundImageId: text(raw.appBackgroundImageId, defaults.appBackgroundImageId),
    readerBackgroundImageId: text(raw.readerBackgroundImageId, defaults.readerBackgroundImageId),
  }
}

export function loadBookManagerPreferences(): BookManagerPreferences {
  try {
    const raw = localStorage.getItem(BOOK_MANAGER_PREFERENCES_KEY)
    return normalizeBookManagerPreferences(raw ? JSON.parse(raw) : {})
  } catch {
    return normalizeBookManagerPreferences()
  }
}

export function saveBookManagerPreferences(patch: Partial<BookManagerPreferences>): BookManagerPreferences {
  const next = normalizeBookManagerPreferences({ ...loadBookManagerPreferences(), ...patch })
  try {
    localStorage.setItem(BOOK_MANAGER_PREFERENCES_KEY, JSON.stringify(next))
  } catch {}
  return next
}
