import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

function findClosingDiv(template: string, openingDivIndex: number): number {
  const divTagPattern = /<\/?div\b[^>]*>/g
  divTagPattern.lastIndex = openingDivIndex
  let depth = 0

  for (let match = divTagPattern.exec(template); match; match = divTagPattern.exec(template)) {
    const tag = match[0]
    if (tag.startsWith('</')) {
      depth--
      if (depth === 0) return match.index
    } else {
      depth++
    }
  }

  return -1
}

describe('BookReaderModal template structure', () => {
  it('keeps the translation popup inside the fullscreen reader shell', () => {
    const source = readFileSync(resolve(__dirname, '../../layout/BookReaderModal.vue'), 'utf8')
    const shellIndex = source.indexOf('<div v-if="visible"')
    const shellCloseIndex = source.lastIndexOf('</Teleport>')
    const translationPopupIndex = source.indexOf('class="trans-popup-layer"')

    expect(shellIndex).toBeGreaterThan(-1)
    expect(shellCloseIndex).toBeGreaterThan(shellIndex)
    expect(translationPopupIndex).toBeGreaterThan(shellIndex)
    expect(translationPopupIndex).toBeLessThan(shellCloseIndex)
  })

  it('routes the right-side AI chat through the migrated bookAI services', () => {
    const source = readFileSync(resolve(__dirname, '../../layout/BookReaderModal.vue'), 'utf8')

    expect(source).toContain('createBookAISettings')
    expect(source).toContain('selectRetrievalBackend')
    expect(source).toContain('loadAIConversationMessages')
    expect(source).toContain('migrateLegacyAIHistory')
    expect(source).toContain('replaceAIConversationMessages')
    expect(source).not.toContain('localStorage.setItem(aiHistoryKey()')
    expect(source).not.toContain('localStorage.getItem(aiHistoryKey()')
  })

  it('keeps migrated chat behavior safe for gateway, retry, and async history loading', () => {
    const source = readFileSync(resolve(__dirname, '../../layout/BookReaderModal.vue'), 'utf8')

    expect(source).toContain('let aiHistoryLoadRequestId = 0')
    expect(source).toContain('aiStatusText')
    expect(source).toContain("cfg.providerName !== 'ai-gateway'")
    expect(source).toContain('findLastIndex')
    expect(source).not.toContain('!cfg.endpoint || !cfg.modelId')
  })

  it('exposes Readest-style AI reader controls in the right settings panel', () => {
    const source = readFileSync(resolve(__dirname, '../../layout/BookReaderModal.vue'), 'utf8')

    // AI settings are available through the global Settings modal (SettingAPI.vue) 
    // and the right-panel still uses settingStore for RAG/spoiler/chunks
    expect(source).toContain('settingStore')
    expect(source).toContain('createBookAISettings')
    expect(source).toContain('isAIConfigured')
    expect(source).toContain('openAIAssistant')
    expect(source).toContain('indexBookForAI')
  })

  it('shows the user chat message before collecting slow reader context', () => {
    const source = readFileSync(resolve(__dirname, '../../layout/BookReaderModal.vue'), 'utf8')
    const askStart = source.indexOf('async function askAI(question: string)')
    const askEnd = source.indexOf('async function retryLastAI()', askStart)
    const askSource = source.slice(askStart, askEnd)
    const appendUserIndex = askSource.indexOf('aiMessages.value = [...aiMessages.value, userMsg]')
    const readContextIndex = askSource.indexOf('getAudioText()')

    expect(appendUserIndex).toBeGreaterThan(-1)
    expect(readContextIndex).toBeGreaterThan(-1)
    expect(appendUserIndex).toBeLessThan(readContextIndex)
  })

  it('guards RAG context extraction with a timeout before chat completion', () => {
    const source = readFileSync(resolve(__dirname, '../../layout/BookReaderModal.vue'), 'utf8')
    const askStart = source.indexOf('async function askAI(question: string)')
    const askEnd = source.indexOf('async function retryLastAI()', askStart)
    const askSource = source.slice(askStart, askEnd)

    expect(askSource).toContain('withRetryAndTimeout(() => bookReader!.getBookAIContextSource')
  })

  it('scrolls iframe reader content when page buttons are used in scroll mode', () => {
    const source = readFileSync(resolve(__dirname, '../../layout/BookReaderModal.vue'), 'utf8')

    expect(source).toContain("import { scrollReaderPageArea } from '../utils/bookReaderScroll'")
    expect(source).toContain('await scrollReaderPageArea(readerContainer.value, bookReader, direction)')
    expect(source).toContain('function syncScrollIframeHeight')
    expect(source).toContain('scrollReaderContentBy(-1)')
    expect(source).toContain('scrollReaderContentBy(1)')
  })

  it('supports Koodo synchronous getProgress when syncing footer page numbers', () => {
    const source = readFileSync(resolve(__dirname, '../../layout/BookReaderModal.vue'), 'utf8')
    const syncStart = source.indexOf('function syncReaderProgress')
    const syncEnd = source.indexOf('function bindReaderIframeEventListeners', syncStart)
    const syncSource = source.slice(syncStart, syncEnd)

    expect(syncSource).toContain('Promise.resolve(rendition.getProgress())')
    expect(syncSource).toContain('normalizeReaderPageProgress')
    expect(syncSource).toContain('estimateReaderPageProgressFromElement')
    expect(syncSource).not.toContain('rendition.getProgress().then')
  })

  it('applies reader margin through live iframe content CSS instead of rerendering the book', () => {
    const source = readFileSync(resolve(__dirname, '../../layout/BookReaderModal.vue'), 'utf8')
    const styleStart = source.indexOf('const readerStageStyle')
    const styleEnd = source.indexOf('const selectionPopupStyle', styleStart)
    const styleSource = source.slice(styleStart, styleEnd)
    const rerenderWatchStart = source.indexOf('// Font/style changes: full re-render to apply')
    const rerenderWatchEnd = source.indexOf('watch([readerBackgroundColor, readerTextColor]', rerenderWatchStart)
    const rerenderWatchSource = source.slice(rerenderWatchStart, rerenderWatchEnd)

    expect(source).toContain("from '../utils/bookReaderLayout'")
    expect(styleSource).toContain('buildReaderStageStyle')
    expect(source).toContain('buildReaderContentMarginCss(readerMargin.value, readerLayoutMode.value)')
    expect(source).toContain("marginStyle.id = 'reader-content-margin-override'")
    expect(source).toContain('watch([readerMargin], () => {')
    expect(source).toContain('applyReaderStyles()')
    expect(rerenderWatchSource).not.toContain('readerMargin,')
  })

  it('lets boxplayer drive single/scroll layouts and only enforces double-page columns', () => {
    const source = readFileSync(resolve(__dirname, '../../layout/BookReaderModal.vue'), 'utf8')
    const readerSource = readFileSync(resolve(__dirname, '../bookReader.ts'), 'utf8')
    const applyStart = source.indexOf('function applyReaderStyles')
    const applyEnd = source.indexOf('function buildCurrentReaderOptions', applyStart)
    const applySource = source.slice(applyStart, applyEnd)

    expect(source).toContain('applyDoublePageCss')
    expect(applySource).toContain('applyDoublePageCss(container, readerLayoutMode.value)')
    // 双页模式仍需注入 column-count: 2 到 documentElement
    expect(readerSource).toContain("doc.documentElement.style.setProperty('column-count', '2', 'important')")
    expect(readerSource).toContain('kookit-double-page-override')
    // 单页/滚动模式不能强制 column-count:1 / overflow:hidden / width:100%（boxplayer
    // 的横向分页和容器滚动会被破坏，造成只显示部分文字）
    expect(readerSource).not.toMatch(/createElement\(['"]style['"]\)[\s\S]{0,80}kookit-single-page-override/)
    expect(readerSource).not.toContain("setProperty('column-count', '1'")
    expect(readerSource).not.toContain("setProperty('column-width', 'auto'")
    expect(readerSource).not.toContain("setProperty('overflow-y', 'hidden', 'important')")
    expect(readerSource).not.toContain('column-count: 1!important')
    expect(applySource).not.toContain("replace(/column-count")
    expect(readerSource).toContain("applyDoublePageCss(options.container, options.readerMode || 'single')")
  })

  it('does not reset custom reader colors when font size changes', () => {
    const source = readFileSync(resolve(__dirname, '../../layout/BookReaderModal.vue'), 'utf8')
    const themeWatchStart = source.indexOf('watch([readerMode')
    const themeWatchEnd = source.indexOf('watch([readerPageWidth]', themeWatchStart)
    const themeWatchSource = source.slice(themeWatchStart, themeWatchEnd)

    expect(themeWatchSource).toContain('readerBackgroundColor.value')
    expect(themeWatchSource).toContain('readerTextColor.value')
    expect(themeWatchSource).not.toContain('fontSize')
  })

  it('enters fullscreen automatically when the reader opens and exits on close', () => {
    const source = readFileSync(resolve(__dirname, '../../layout/BookReaderModal.vue'), 'utf8')
    const windowSource = readFileSync(resolve(__dirname, '../../../electron/main/core/window.ts'), 'utf8')
    const visibleWatchStart = source.indexOf('watch(\n  () => props.visible')
    const visibleWatchEnd = source.indexOf('watch(\n  () => props.book?.id', visibleWatchStart)
    const visibleWatchSource = source.slice(visibleWatchStart, visibleWatchEnd)
    const closeStart = source.indexOf('function close()')
    const closeEnd = source.indexOf('const isFullscreen', closeStart)
    const closeSource = source.slice(closeStart, closeEnd)

    expect(source).toContain('function enterReaderFullscreen')
    expect(source).toContain('function exitReaderFullscreen')
    expect(source).toContain("shouldEnter ? 'enterfullscreen' : 'exitfullscreen'")
    expect(visibleWatchSource).toContain('enterReaderFullscreen()')
    expect(closeSource).toContain('exitReaderFullscreen()')
    expect(source).toContain("document.addEventListener('fullscreenchange', syncFullscreenState)")
    expect(windowSource).toContain("data.cmd === 'enterfullscreen'")
    expect(windowSource).toContain("currentWin.setFullScreen(true)")
    expect(windowSource).toContain("data.cmd === 'exitfullscreen'")
    expect(windowSource).toContain("currentWin.setFullScreen(false)")
  })

  it('matches Koodo keyboard behavior in scroll mode', () => {
    const source = readFileSync(resolve(__dirname, '../../layout/BookReaderModal.vue'), 'utf8')
    const keyStart = source.indexOf('function handleKeyDown')
    const keyEnd = source.indexOf("} else if (key === 'escape')", keyStart)
    const keySource = source.slice(keyStart, keyEnd)
    const iframeKeyStart = source.indexOf('function handleReaderIframeKeyDown')
    const iframeKeyEnd = source.indexOf('let lastReaderWheelAt', iframeKeyStart)
    const iframeKeySource = source.slice(iframeKeyStart, iframeKeyEnd)

    expect(source).toContain('function scrollReaderByNativeArrow')
    expect(keySource).toContain('scrollReaderByNativeArrow(key ===')
    expect(keySource).toContain('scrollPageUp()')
    expect(keySource).toContain('scrollPageDown()')
    expect(keySource).toContain("key === 'arrowleft' || key === 'pageup'")
    expect(keySource).toContain("key === 'arrowright' || key === 'pagedown' || key === ' '")
    expect(keySource).not.toContain("key === 'arrowleft' || key === 'arrowup' || key === 'pageup'")
    expect(keySource).not.toContain("key === 'arrowright' || key === 'arrowdown' || key === 'pagedown' || key === ' '")
    expect(keySource).not.toContain('if (isScroll) return // native scroll')
    expect(iframeKeySource).toContain('handleKeyDown(event)')
    expect(iframeKeySource).not.toContain('滚动模式下让浏览器处理原生滚动按键')
  })
})
