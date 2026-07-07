import { getFormat } from './bookReaderCapabilities'
import { normalizeReaderPosition, type BookReaderPosition } from './bookReaderState'
import { buildInitialReaderPosition, normalizeReaderPercentage, serializeReaderPositionForJump } from './bookReaderParity'
import { buildBookHighlight, toBookNote } from './bookNotes'
import { buildBookBookmark } from './bookBookmarks'
import type { IBookItem } from '../types/book'
import type { IBookNote } from '../types/bookNote'
import type { IBookBookmark } from '../types/bookBookmark'
import { extractTextFromHtml, type BookAIContextSource } from './bookAI'

export interface BookReaderOptions {
  sourceUrl: string
  ext: string
  container: HTMLElement
  initialPosition?: BookReaderPosition
  readerMode?: 'single' | 'double' | 'scroll'
  isDarkMode?: boolean
  fontSize?: number
  isIndent?: boolean
  isHyphenation?: boolean
  isBionic?: boolean
  paraSpacingValue?: string
  lineHeight?: string
  textAlign?: string
  backgroundColor?: string
  textColor?: string
  fontFamily?: string
  subFontFamily?: string
  margin?: number
  letterSpacing?: number
  isBold?: boolean
  isItalic?: boolean
  isUnderline?: boolean
  isShadow?: boolean
  isSliding?: boolean
  isOrphanWidow?: boolean
  isAllowScript?: boolean
  isAutoScroll?: boolean
  isOverwriteText?: boolean
  isOverwriteLink?: boolean
  scale?: number
  bookLayout?: string
  convertChinese?: string
  fullTranslationMode?: string
  textOrientation?: string
  customCSS?: string
}

export interface BookChapter {
  label: string
  href?: string
  id?: string
  subitems?: BookChapter[]
  [key: string]: unknown
}

export interface BookSearchResult {
  id: string
  excerpt: string
  chapterTitle: string
  keyword: string
  position: BookReaderPosition
}

export interface BookReferResult {
  handled: boolean
  externalHref?: string
  footnoteHtml?: string
  href?: string
  isJump?: boolean
}

export interface BookReaderHandle {
  rendition: any
  prev: () => Promise<void>
  next: () => Promise<void>
  goToChapter: (index: number) => Promise<void>
  goToPercentage: (percentage: number) => Promise<void>
  search: (query: string) => Promise<BookSearchResult[]>
  goToSearchResult: (result: BookSearchResult) => Promise<void>
  goToNote: (note: IBookNote) => Promise<void>
  goToBookmark: (bookmark: IBookBookmark) => Promise<void>
  goToPosition: (position: BookReaderPosition) => Promise<void>
  goToHrefInDocument: (href: string, doc: Document) => Promise<void>
  handleLinkClick: (event: Event) => Promise<BookReferResult | null>
  renderHighlights: (notes: IBookNote[]) => Promise<void>
  createHighlight: (book: IBookItem, noteText?: string, color?: number) => Promise<IBookNote | null>
  updateHighlight: (note: IBookNote) => Promise<void>
  removeHighlight: (note: IBookNote) => Promise<void>
  createBookmark: (book: IBookItem) => Promise<IBookBookmark>
  getVisibleText: () => Promise<string>
  getAudioText: () => Promise<string>
  getBookAIContextSource: (book: IBookItem) => Promise<BookAIContextSource>
  highlightAudioText: (text: string) => void
  nextChapter: () => Promise<void>
  getChapters: () => BookChapter[]
  getPosition: () => BookReaderPosition
  recordPosition: () => Promise<BookReaderPosition>
  getProgressText: () => string
  applyStyles: () => void
  updateColors: (bg: string, fg: string) => void
  getBatchTransTexts: () => Promise<string[]>
  handleBatchTransResult: (texts: string[], translations: string[]) => void
  getBookCover: () => Promise<string | null>
  destroy: () => void
  _ReaderKit: RenderKitModule
  _contentBuffer: ArrayBuffer
}

type RenderKitModule = typeof import('../vendor/reader/readerkit.min.js')
const READER_RENDER_SETTLE_TIMEOUT_MS = 1800

function createReaderOptions(options: BookReaderOptions) {
  const isScroll = options.readerMode === 'scroll'
  return {
    format: getFormat(options.ext),
    readerMode: options.readerMode || 'single',
    flow: isScroll ? 'scrolled-doc' : 'paginated',
    spread: isScroll ? 'none' : (options.readerMode === 'double' ? 'auto' : 'none'),
    charset: 'utf-8',
    animation: '',
    convertChinese: options.convertChinese || 'none',
    bookLayout: options.bookLayout || '',
    parserRegex: '',
    fullTranslationMode: options.fullTranslationMode || 'no',
    textOrientation: options.textOrientation || '',
    isDarkMode: options.isDarkMode ? 'yes' : 'no',
    backgroundColor: options.backgroundColor || '',
    textColor: options.textColor || '',
    isMobile: 'no',
    isIndent: options.isIndent === false ? 'no' : 'yes',
    isHyphenation: options.isHyphenation ? 'yes' : 'no',
    isStartFromEven: 'no',
    isAllowScript: options.isAllowScript ? 'yes' : 'no',
    isBionic: options.isBionic ? 'yes' : 'no',
    password: '',
    scale: options.scale || 1,
    fontSize: options.fontSize || 18,
    lineHeight: options.lineHeight || '1.5',
    textAlign: options.textAlign || '',
    isConvertPDF: 'no',
    ocrLang: 'chi_sim',
    externalWorker: {},
    ocrEngine: 'tesseract',
    serverRegion: 'global',
    paraSpacing: options.paraSpacingValue || '24',
    paraSpacingValue: options.paraSpacingValue || '24',
    titleSizeValue: '1.2',
    isScannedPDF: 'no'
  }
}

export function createReaderStyleConfig(options: BookReaderOptions) {
  const readerConfig: Record<string, string> = {
    readerMode: options.readerMode || 'double',
    backgroundColor: options.backgroundColor || '',
    textColor: options.textColor || '',
    fontSize: String(options.fontSize || 18),
    lineHeight: options.lineHeight || '1.5',
    textAlign: options.textAlign || '',
    isIndent: options.isIndent === false ? 'no' : 'yes',
    isHyphenation: options.isHyphenation ? 'yes' : 'no',
    isBionic: options.isBionic ? 'yes' : 'no',
    paraSpacing: options.paraSpacingValue || '24',
    isDarkMode: options.isDarkMode ? 'yes' : 'no',
    isOverwriteText: options.isOverwriteText ? 'yes' : (options.textColor ? 'yes' : 'no'),
    isOverwriteLink: options.isOverwriteLink ? 'yes' : 'no',
    appSkin: options.isDarkMode ? 'night' : 'default',
    isOSNight: 'no',
    fontFamily: options.fontFamily || '',
    subFontFamily: options.subFontFamily || '',
    isBold: options.isBold ? 'yes' : 'no',
    isItalic: options.isItalic ? 'yes' : 'no',
    isUnderline: options.isUnderline ? 'yes' : 'no',
    isShadow: options.isShadow ? 'yes' : 'no',
    isSliding: options.isSliding ? 'yes' : 'no',
    margin: String(options.margin ?? 0),
    letterSpacing: String(options.letterSpacing ?? 0)
  }
  return {
    getReaderConfig: (key: string) => readerConfig[key] || '',
    getAllListConfig: () => [],
    getObjectConfig: () => ({})
  }
}

export function applyReaderDefaultCss(container: HTMLElement, ReaderKit: RenderKitModule, options: BookReaderOptions) {
  const styleHelper = (ReaderKit as any).StyleHelper
  if (!styleHelper?.getDefaultCss) return false
  const css = styleHelper.getDefaultCss(createReaderStyleConfig(options), '')
  if (!css) return false
  for (const iframe of Array.from(container.querySelectorAll('iframe'))) {
    const doc = iframe.contentDocument
    if (!doc?.head) continue
    let style = doc.getElementById('default-style') as HTMLStyleElement | null
    if (!style) {
      style = doc.createElement('style')
      style.id = 'default-style'
      doc.head.appendChild(style)
    }
    style.textContent = css
  }
  return true
}

export function applyDoublePageCss(container: HTMLElement, readerMode: string) {
  const iframes = container.querySelectorAll('iframe')
  const isDouble = readerMode === 'double'
  for (const iframe of Array.from(iframes)) {
    const doc = iframe.contentDocument
    if (!doc?.body || !doc?.head) continue
    if (!isDouble) {
      const oldDouble = doc.getElementById('kookit-double-page-override')
      if (oldDouble) oldDouble.remove()
      const oldSingle = doc.getElementById('kookit-single-page-override')
      if (oldSingle) oldSingle.remove()
      const cleanupProps = ['column-count', 'column-width', 'column-gap', 'column-fill', 'width', 'max-width', 'overflow-x', 'overflow-y'] as const
      for (const el of [doc.documentElement, doc.body]) {
        for (const prop of cleanupProps) el.style.removeProperty(prop)
      }
      if (doc.body) {
        for (const child of Array.from(doc.body.children)) {
          const el = child as HTMLElement
          for (const prop of ['column-count', 'column-width', 'column-gap', 'column-fill'] as const) {
            el.style.removeProperty(prop)
          }
        }
      }
      continue
    }
    const single = doc.getElementById('kookit-single-page-override')
    if (single) single.remove()
    const containerWidth = container.clientWidth || 800
    const gapRaw = Math.floor(containerWidth / 12)
    const gap = gapRaw % 2 === 0 ? gapRaw : gapRaw - 1
    const colWidth = (containerWidth - gap) / 2
    doc.documentElement.style.setProperty('column-count', '2', 'important')
    doc.documentElement.style.setProperty('column-width', `${colWidth}px`, 'important')
    doc.documentElement.style.setProperty('column-gap', `${gap}px`, 'important')
    let styleEl = doc.getElementById('kookit-double-page-override') as HTMLStyleElement | null
    if (!styleEl) {
      styleEl = doc.createElement('style')
      styleEl.id = 'kookit-double-page-override'
      doc.head.appendChild(styleEl)
    }
    styleEl.textContent = `html{column-count:2!important;column-width:${colWidth}px!important;column-gap:${gap}px!important}`
  }
}

async function fetchBookBuffer(sourceUrl: string): Promise<ArrayBuffer> {
  const resp = await fetch(sourceUrl)
  if (!resp.ok) throw new Error(`书籍加载失败 HTTP ${resp.status}`)
  return resp.arrayBuffer()
}

function readProgressText(rendition: any): string {
  try {
    const position = rendition?.getPosition?.()
    if (position?.percentage !== undefined && position.percentage !== '') {
      const numeric = Number(position.percentage)
      if (Number.isFinite(numeric)) return `${Math.round(numeric * 100)}%`
    }
    if (position?.page) return String(position.page)
    if (position?.chapterTitle) return String(position.chapterTitle)
  } catch {}
  return '-'
}

export async function readRecordedReaderPosition(rendition: any): Promise<BookReaderPosition> {
  await rendition?.record?.()
  return normalizeReaderPosition(rendition?.getPosition?.())
}

function flattenChapters(chapters: any[]): BookChapter[] {
  const flat: BookChapter[] = []
  const walk = (items: any[]) => {
    for (const item of items || []) {
      flat.push(item)
      if (Array.isArray(item?.subitems)) walk(item.subitems)
    }
  }
  walk(chapters)
  return flat
}

function stripHtml(input = ''): string {
  return input
    .replace(/<[^>]*>/g, '')
    .replace(/\s+/g, ' ')
    .trim()
}

async function getChapterSnapshots(rendition: any, chapters: BookChapter[], visibleTextFallback: string) {
  const docs = rendition.getChapterDoc?.() || []
  const snapshots = []
  if (Array.isArray(docs) && docs.length) {
    for (let index = 0; index < docs.length; index++) {
      const raw = docs[index]
      const doc = raw?.documentElement || raw?.body ? (raw as Document) : null
      const html = doc?.body?.innerHTML || doc?.documentElement?.innerHTML || String(raw?.html || raw?.content || '')
      const text = doc ? stripHtml(doc.body?.textContent || doc.documentElement?.textContent || '') : extractTextFromHtml(html)
      if (!text) continue
      snapshots.push({
        index,
        title: chapters[index]?.label || `章节 ${index + 1}`,
        text,
        href: chapters[index]?.href
      })
    }
  }
  if (!snapshots.length && visibleTextFallback) {
    const position = normalizeReaderPosition(rendition.getPosition?.())
    const index = Number(position.chapterDocIndex || 0)
    snapshots.push({
      index,
      title: chapters[index]?.label || position.chapterTitle || '当前章节',
      text: visibleTextFallback,
      href: chapters[index]?.href
    })
  }
  return snapshots
}

function parseSearchPosition(raw: any): BookReaderPosition {
  try {
    const parsed = typeof raw === 'string' ? JSON.parse(raw) : raw
    return normalizeReaderPosition(parsed)
  } catch {
    return {}
  }
}

function getSelectedText(container: HTMLElement): string {
  const docs = Array.from(container.querySelectorAll('iframe'))
    .map((iframe) => iframe.contentDocument)
    .filter(Boolean) as Document[]
  for (const doc of docs) {
    const text = doc.getSelection?.()?.toString() || ''
    if (text.trim()) return text
  }
  return window.getSelection?.()?.toString() || ''
}

function findDocumentNodeByHref(doc: Document, href: string): Element | null {
  const id = href.split('#').reverse()[0]
  if (!id) return null
  const escape = (globalThis as any).CSS?.escape
  const selector = `#${escape ? escape(id) : id.replace(/"/g, '\\"')}`
  try {
    return doc.body.querySelector(selector)
  } catch {
    return null
  }
}

async function getVisibleText(rendition: any): Promise<string> {
  try {
    const text = await rendition.visibleText?.()
    return Array.isArray(text) ? text.join(' ') : String(text || '')
  } catch {
    return ''
  }
}

async function getAudioText(rendition: any): Promise<string> {
  try {
    const text = await rendition.audioText?.()
    if (Array.isArray(text)) {
      return text.map((item) => (typeof item === 'string' ? item : item?.text || '')).join(' ')
    }
    if (text) return typeof text === 'string' ? text : String(text?.text || text)
  } catch {}
  return getVisibleText(rendition)
}

export async function waitForReaderRender(renderResult: unknown): Promise<void> {
  if (!renderResult || typeof (renderResult as PromiseLike<void>).then !== 'function') return
  let timer: ReturnType<typeof setTimeout> | undefined
  await Promise.race([
    renderResult as PromiseLike<void>,
    new Promise<void>((resolve) => {
      timer = setTimeout(resolve, READER_RENDER_SETTLE_TIMEOUT_MS)
    })
  ]).finally(() => {
    if (timer) clearTimeout(timer)
  })
}

export function normalizeReaderSearchResults(items: any[]): BookSearchResult[] {
  return (items || []).map((item, index) => {
    const rawPosition = (() => {
      try {
        return typeof item?.cfi === 'string' ? JSON.parse(item.cfi) : item?.cfi
      } catch {
        return {}
      }
    })()
    const position = parseSearchPosition(rawPosition)
    const idSeed = item?.text || item?.excerpt || item?.cfi || index
    return {
      id: `${index}-${idSeed}`,
      excerpt: stripHtml(item?.excerpt || item?.text || ''),
      chapterTitle: String(rawPosition?.chapterTitle || position.chapterTitle || ''),
      keyword: String(rawPosition?.keyword || item?.keyword || ''),
      position
    }
  })
}

export async function createBookReader(options: BookReaderOptions, cachedContent?: ArrayBuffer): Promise<BookReaderHandle> {
  const ReaderKit: RenderKitModule = await import('../vendor/reader/readerkit.min.js')
  const content = cachedContent || await fetchBookBuffer(options.sourceUrl)
  const readerOptions = createReaderOptions(options)
  const rendition = ReaderKit.BookHelper.getRendition(content, readerOptions, ReaderKit)
  options.container.innerHTML = ''
  if (readerOptions.format === 'TXT') {
    await waitForReaderRender(rendition.renderTo(options.container, options.initialPosition || {}))
  } else {
    await waitForReaderRender(rendition.renderTo(options.container))
  }
  const chapterDocs = rendition.getChapterDoc?.() || []
  if (chapterDocs.length > 0) {
    await rendition.goToPosition?.(serializeReaderPositionForJump(buildInitialReaderPosition(options.initialPosition)))
  }
  applyReaderDefaultCss(options.container, ReaderKit, options)
  applyDoublePageCss(options.container, options.readerMode || 'single')

  return {
    rendition,
    prev: async () => {
      await rendition.prev?.()
    },
    next: async () => {
      await rendition.next?.()
    },
    goToChapter: async (index: number) => {
      await rendition.goToChapterIndex?.(index)
    },
    goToPercentage: async (percentage: number) => {
      await rendition.goToPercentage?.(normalizeReaderPercentage(percentage))
    },
    search: async (query: string) => normalizeReaderSearchResults((await rendition.doSearch?.(query)) || []),
    goToSearchResult: async (result: BookSearchResult) => {
      await rendition.goToPosition?.(serializeReaderPositionForJump(result.position))
      if (result.keyword) {
        rendition.highlightSearchNode?.(result.keyword, 'background: #f3a6a68c;')
      }
    },
    goToNote: async (note: IBookNote) => {
      await rendition.goToPosition?.(serializeReaderPositionForJump(note.position))
    },
    goToBookmark: async (bookmark: IBookBookmark) => {
      await rendition.goToPosition?.(serializeReaderPositionForJump(bookmark.position))
    },
    goToPosition: async (position: BookReaderPosition) => {
      await rendition.goToPosition?.(serializeReaderPositionForJump(position))
    },
    goToHrefInDocument: async (href: string, doc: Document) => {
      await rendition.goToNode?.(findDocumentNodeByHref(doc, href) || doc.body)
    },
    handleLinkClick: async (event: Event) => {
      const href = rendition.getTargetHref?.(event)
      if (!href) return null
      const result = await rendition.handleLinkJump?.(href, event)
      if (!result?.handled) return null
      if (result.external) return { handled: true, externalHref: href }
      const refer: BookReferResult = {
        handled: true,
        href: result.href || '',
        isJump: !!result.isJump
      }
      if (result.isShowMenu) {
        const footnote = await rendition.getFootnoteContent?.(result.node)
        if (!footnote?.handled) return refer
        refer.footnoteHtml = String(footnote.content || '')
      }
      return refer
    },
    renderHighlights: async (notes: IBookNote[]) => {
      if (!notes.length) return
      await rendition.renderHighlighters?.(notes.map(toBookNote), () => {})
    },
    createHighlight: async (book: IBookItem, noteText?: string, color = 0) => {
      const text = getSelectedText(options.container)
      if (!text.trim()) return null
      const position = normalizeReaderPosition(rendition.getPosition?.())
      const chapterIndex = Number(position.chapterDocIndex || 0)
      const range = JSON.stringify((await rendition.getHightlightCoords?.(chapterIndex)) || [])
      const note = buildBookHighlight({
        book,
        text,
        position,
        range,
        note: noteText,
        color
      })
      await rendition.createOneNote?.(toBookNote(note), () => {})
      return note
    },
    updateHighlight: async (note: IBookNote) => {
      await rendition.removeOneNote?.(note.id, note.chapter_index)
      await rendition.createOneNote?.(toBookNote(note), () => {})
    },
    removeHighlight: async (note: IBookNote) => {
      await rendition.removeOneNote?.(note.id, note.chapter_index)
    },
    createBookmark: async (book: IBookItem) => {
      const position = normalizeReaderPosition(rendition.getPosition?.())
      return buildBookBookmark({
        book,
        position,
        visibleText: await getVisibleText(rendition)
      })
    },
    getVisibleText: async () => getVisibleText(rendition),
    getAudioText: async () => getAudioText(rendition),
    getBookCover: async () => {
      try {
        const meta = await rendition.getMetadata?.()
        return meta?.cover || null
      } catch {
        return null
      }
    },
    getBookAIContextSource: async (book: IBookItem) => {
      const chapters = flattenChapters(rendition.getChapter?.() || [])
      const audioText = await getAudioText(rendition)
      const visibleText = await getVisibleText(rendition)
      return {
        bookId: book.id,
        sourceHash: `${book.id}:${book.updated_at || ''}:${book.size || 0}`,
        title: book.title || book.file_name || '未知书籍',
        author: book.author || '',
        chapters: await getChapterSnapshots(rendition, chapters, audioText || visibleText),
        currentPosition: normalizeReaderPosition(rendition.getPosition?.()),
        visibleText,
        chapterText: audioText || visibleText
      }
    },
    highlightAudioText: (text: string) => {
      try {
        rendition.highlightAudioNode?.(text, 'background: #f3a6a68c;')
      } catch {}
    },
    nextChapter: async () => {
      if (rendition.nextChapter) await rendition.nextChapter()
      else await rendition.next?.()
    },
    getChapters: () => flattenChapters(rendition.getChapter?.() || []),
    getPosition: () => normalizeReaderPosition(rendition.getPosition?.()),
    recordPosition: () => readRecordedReaderPosition(rendition),
    getProgressText: () => readProgressText(rendition),
    applyStyles: () => applyReaderDefaultCss(options.container, ReaderKit, options),
    updateColors: (bg: string, fg: string) => {
      options.backgroundColor = bg
      options.textColor = fg
      // Re-apply CSS to iframe without reload
      try {
        const iframe = options.container.querySelector('iframe')
        if (iframe?.contentDocument) {
          const styleEl = iframe.contentDocument.querySelector('#default-style')
          if (styleEl) {
            const styleHelper = (ReaderKit as any).StyleHelper
            if (styleHelper?.getDefaultCss) {
              const updatedOptions = { ...options, backgroundColor: bg, textColor: fg }
              styleEl.textContent = styleHelper.getDefaultCss(createReaderStyleConfig(updatedOptions), '')
            }
          }
        }
      } catch {}
    },
    getBatchTransTexts: async () => {
      try {
        return (await rendition.getBatchTransTexts?.()) || []
      } catch {
        return []
      }
    },
    handleBatchTransResult: (texts: string[], translations: string[]) => {
      try {
        rendition.handleBatchTransResult?.(texts, translations)
      } catch {}
    },
    destroy: () => {
      try {
        rendition.removeContent?.()
      } catch {}
      options.container.innerHTML = ''
    },
    _ReaderKit: ReaderKit,
    _contentBuffer: content
  }
}
