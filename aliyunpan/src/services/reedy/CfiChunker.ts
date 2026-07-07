import type { ChunkRow } from './types'
import { CHUNK_OPTIONS } from './types'

export interface ChunkOptions {
  maxChunkSize: number
  minChunkSize: number
  overlapSize: number
  breakSearchRange: number
}

const DEFAULT_OPTIONS: ChunkOptions = {
  maxChunkSize: 500,
  minChunkSize: 100,
  overlapSize: 50,
  breakSearchRange: 50
}

const SKIP_TAGS = new Set(['SCRIPT', 'STYLE', 'NOSCRIPT', 'TEMPLATE'])

interface TextSlice {
  node: Text
  cumStart: number
}

function collectTextNodes(doc: Document): { slices: TextSlice[]; flatText: string } {
  const body = doc.body ?? doc.documentElement
  if (!body) return { slices: [], flatText: '' }

  const walker = doc.createTreeWalker(body, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      let p: Node | null = node.parentNode
      while (p && p.nodeType === 1) {
        const el = p as Element
        if (SKIP_TAGS.has(el.tagName)) return NodeFilter.FILTER_REJECT
        if (el.classList?.contains('cfi-inert')) return NodeFilter.FILTER_REJECT
        p = p.parentNode
      }
      return (node.nodeValue ?? '').length > 0 ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP
    }
  })

  const slices: TextSlice[] = []
  const parts: string[] = []
  let cum = 0
  let n: Node | null = walker.nextNode()
  while (n) {
    const text = (n as Text).nodeValue ?? ''
    slices.push({ node: n as Text, cumStart: cum })
    parts.push(text)
    cum += text.length
    n = walker.nextNode()
  }
  return { slices, flatText: parts.join('') }
}

function offsetToNode(slices: TextSlice[], offset: number): { node: Text; offset: number } | null {
  if (slices.length === 0) return null
  let lo = 0
  let hi = slices.length - 1
  while (lo < hi) {
    const mid = (lo + hi + 1) >>> 1
    if (slices[mid]!.cumStart <= offset) lo = mid
    else hi = mid - 1
  }
  const slice = slices[lo]!
  const within = offset - slice.cumStart
  const nodeLen = (slice.node.nodeValue ?? '').length
  return { node: slice.node, offset: Math.min(within, nodeLen) }
}

function findBreakPoint(text: string, targetPos: number, searchRange: number): number {
  const start = Math.max(0, targetPos - searchRange)
  const end = Math.min(text.length, targetPos + searchRange)
  const window = text.slice(start, end)

  const paragraphBreak = window.lastIndexOf('\n\n')
  if (paragraphBreak !== -1 && paragraphBreak > searchRange / 2) {
    return start + paragraphBreak + 2
  }
  const sentenceBreak = window.lastIndexOf('. ')
  if (sentenceBreak !== -1 && sentenceBreak > searchRange / 2) {
    return start + sentenceBreak + 2
  }
  const wordBreak = window.lastIndexOf(' ')
  if (wordBreak !== -1) {
    return start + wordBreak + 1
  }
  return targetPos
}

function tokenCount(text: string): number {
  const trimmed = text.trim()
  if (trimmed.length === 0) return 0
  return trimmed.split(/\s+/).length
}

export function chunkSection(doc: Document, sectionIndex: number, chapterTitle: string, bookHash: string, options?: Partial<ChunkOptions>): ChunkRow[] {
  const opts: ChunkOptions = { ...DEFAULT_OPTIONS, ...options }
  const { slices, flatText } = collectTextNodes(doc)
  if (flatText.trim().length === 0 || slices.length === 0) return []

  const totalLen = flatText.length
  if (totalLen < opts.minChunkSize) {
    return buildChunks([{ start: 0, end: totalLen }], flatText, bookHash, sectionIndex, chapterTitle)
  }

  const windows: Array<{ start: number; end: number }> = []
  let cursor = 0
  while (cursor < totalLen) {
    const targetEnd = cursor + opts.maxChunkSize
    if (targetEnd >= totalLen) {
      windows.push({ start: cursor, end: totalLen })
      break
    }
    const snappedEnd = findBreakPoint(flatText, targetEnd, opts.breakSearchRange)
    const end = snappedEnd > cursor ? snappedEnd : Math.min(totalLen, cursor + opts.maxChunkSize)
    windows.push({ start: cursor, end })
    cursor = end > opts.overlapSize ? end - opts.overlapSize : end
    if (cursor >= totalLen) break
  }

  return buildChunks(windows, flatText, bookHash, sectionIndex, chapterTitle)
}

export function chunkPlainText(text: string, sectionIndex: number, chapterTitle: string, bookHash: string, options?: Partial<ChunkOptions>): ChunkRow[] {
  const opts: ChunkOptions = { ...DEFAULT_OPTIONS, ...options }
  const trimmed = text.trim()
  if (!trimmed) return []

  if (trimmed.length < opts.minChunkSize) {
    return buildChunks([{ start: 0, end: trimmed.length }], trimmed, bookHash, sectionIndex, chapterTitle)
  }

  const windows: Array<{ start: number; end: number }> = []
  let cursor = 0
  while (cursor < trimmed.length) {
    const targetEnd = cursor + opts.maxChunkSize
    if (targetEnd >= trimmed.length) {
      windows.push({ start: cursor, end: trimmed.length })
      break
    }
    const snappedEnd = findBreakPoint(trimmed, targetEnd, opts.breakSearchRange)
    const end = snappedEnd > cursor ? snappedEnd : Math.min(trimmed.length, cursor + opts.maxChunkSize)
    windows.push({ start: cursor, end })
    cursor = end > opts.overlapSize ? end - opts.overlapSize : end
    if (cursor >= trimmed.length) break
  }

  return buildChunks(windows, trimmed, bookHash, sectionIndex, chapterTitle)
}

function buildChunks(windows: Array<{ start: number; end: number }>, flatText: string, bookHash: string, sectionIndex: number, chapterTitle: string): ChunkRow[] {
  const out: ChunkRow[] = []
  let position = 0
  for (const w of windows) {
    const sliceText = flatText.slice(w.start, w.end).trim()
    if (sliceText.length === 0) continue
    out.push({
      id: `${bookHash}-${sectionIndex}-${position}`,
      book_hash: bookHash,
      section_index: sectionIndex,
      chapter_title: chapterTitle,
      start_cfi: String(w.start),
      end_cfi: String(w.end),
      position_index: position,
      text: sliceText,
      token_count: tokenCount(sliceText)
    })
    position++
  }
  return out
}
