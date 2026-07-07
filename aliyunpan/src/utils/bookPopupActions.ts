export type BookPopupActionKey =
  | 'note'
  | 'highlight'
  | 'translation'
  | 'copy'
  | 'search-book'
  | 'dict'
  | 'browser'
  | 'speaker'
  | 'speech-start'
  | 'assistant'

export interface ReaderPopupAction {
  key: BookPopupActionKey
  title: string
  defaultEnabled: boolean
}

export interface ReaderHighlightColor {
  index: number
  value: string
  mode: 'background' | 'line'
}

const READER_POPUP_OPTION_LIMIT = 8

export const POPUP_ACTIONS: ReaderPopupAction[] = [
  { key: 'note', title: 'Take a note', defaultEnabled: true },
  { key: 'highlight', title: 'Highlight', defaultEnabled: true },
  { key: 'translation', title: 'Translate', defaultEnabled: true },
  { key: 'copy', title: 'Copy', defaultEnabled: true },
  { key: 'search-book', title: 'Search in the Book', defaultEnabled: true },
  { key: 'dict', title: 'Dictionary', defaultEnabled: true },
  { key: 'browser', title: 'Search on the Internet', defaultEnabled: true },
  { key: 'speaker', title: 'Speak the text', defaultEnabled: true },
  { key: 'speech-start', title: 'Read from here', defaultEnabled: false },
  { key: 'assistant', title: 'Ask AI', defaultEnabled: false }
]

export const HIGHLIGHT_COLORS: ReaderHighlightColor[] = [
  { index: 0, value: '#FBF1D1', mode: 'background' },
  { index: 1, value: '#EFEEB0', mode: 'background' },
  { index: 2, value: '#CAEFC9', mode: 'background' },
  { index: 3, value: '#76BEE9', mode: 'background' },
  { index: 4, value: '#FF0000', mode: 'line' },
  { index: 5, value: '#000080', mode: 'line' },
  { index: 6, value: '#0000FF', mode: 'line' },
  { index: 7, value: '#2EFF2E', mode: 'line' }
]

export function buildPopupActions(enabledKeys?: BookPopupActionKey[]): ReaderPopupAction[] {
  const enabled = enabledKeys?.length
    ? enabledKeys
    : POPUP_ACTIONS.filter((item) => item.defaultEnabled).map((item) => item.key)
  const enabledSet = new Set(enabled)
  return POPUP_ACTIONS.filter((item) => enabledSet.has(item.key)).slice(0, READER_POPUP_OPTION_LIMIT)
}

export function normalizeReaderHighlightColor(value: unknown): number {
  const numeric = Number(value)
  if (!Number.isInteger(numeric)) return 0
  return HIGHLIGHT_COLORS.some((item) => item.index === numeric) ? numeric : 0
}

export function getReaderHighlightColor(value: unknown): ReaderHighlightColor {
  const index = normalizeReaderHighlightColor(value)
  return HIGHLIGHT_COLORS.find((item) => item.index === index) || HIGHLIGHT_COLORS[0]
}

const SEARCH_ENGINES: Record<string, string> = {
  google: 'https://www.google.com/search?q=',
  baidu: 'https://www.baidu.com/s?wd=',
  bing: 'https://www.bing.com/search?q=',
  duckduckgo: 'https://duckduckgo.com/?q=',
}

export type SearchEngine = keyof typeof SEARCH_ENGINES

export function buildBrowserSearchUrl(text: string, engine: SearchEngine = 'google'): string {
  const query = text.trim()
  if (!query) return ''
  const base = SEARCH_ENGINES[engine] || SEARCH_ENGINES.google
  return base + encodeURIComponent(query)
}

export function normalizePopupNoteText(text: string): string {
  return text.trim()
}
