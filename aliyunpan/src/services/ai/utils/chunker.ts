import type { TextChunk } from '../types'

export const SIZE_PER_PAGE = 1500

const DEFAULT_OPTIONS = {
  maxChunkSize: 500,
  overlapSize: 50,
  minChunkSize: 100,
}

function normalizeText(input: string): string {
  return input.replace(/\s+/g, ' ').trim()
}

function findBreakPoint(text: string, targetPos: number, searchRange = 50): number {
  const start = Math.max(0, targetPos - searchRange)
  const end = Math.min(text.length, targetPos + searchRange)
  const searchText = text.slice(start, end)
  const paragraphBreak = searchText.lastIndexOf('\n\n')
  if (paragraphBreak !== -1 && paragraphBreak > searchRange / 2) return start + paragraphBreak + 2
  const sentenceBreak = Math.max(
    searchText.lastIndexOf('. '), searchText.lastIndexOf('。'),
    searchText.lastIndexOf('！'), searchText.lastIndexOf('？')
  )
  if (sentenceBreak !== -1 && sentenceBreak > searchRange / 2) return start + sentenceBreak + 1
  const wordBreak = searchText.lastIndexOf(' ')
  if (wordBreak !== -1) return start + wordBreak + 1
  return targetPos
}

export function chunkSection(
  text: string,
  sectionIndex: number,
  chapterTitle: string,
  bookHash: string,
  cumulativeSizeBeforeSection = 0,
  options?: Partial<typeof DEFAULT_OPTIONS>
): TextChunk[] {
  const opts = { ...DEFAULT_OPTIONS, ...(options || {}) }
  const cleanText = normalizeText(text)
  if (!cleanText) return []
  if (cleanText.length < opts.minChunkSize) {
    return [{
      id: `${bookHash}:${sectionIndex}:0`,
      bookHash,
      sectionIndex,
      chapterTitle,
      text: cleanText,
      pageNumber: Math.floor(cumulativeSizeBeforeSection / SIZE_PER_PAGE),
    }]
  }

  const chunks: TextChunk[] = []
  let position = 0
  let chunkIndex = 0
  while (position < cleanText.length) {
    let chunkEnd = position + opts.maxChunkSize
    if (chunkEnd >= cleanText.length) {
      const remaining = cleanText.slice(position).trim()
      if (remaining.length >= opts.minChunkSize || chunks.length === 0) {
        chunks.push({
          id: `${bookHash}:${sectionIndex}:${chunkIndex}`,
          bookHash,
          sectionIndex,
          chapterTitle,
          text: remaining,
          pageNumber: Math.floor((cumulativeSizeBeforeSection + position) / SIZE_PER_PAGE),
        })
      } else if (remaining && chunks.length > 0) {
        chunks[chunks.length - 1].text += ` ${remaining}`
      }
      break
    }
    chunkEnd = findBreakPoint(cleanText, chunkEnd)
    const chunkText = cleanText.slice(position, chunkEnd).trim()
    if (chunkText.length >= opts.minChunkSize) {
      chunks.push({
        id: `${bookHash}:${sectionIndex}:${chunkIndex}`,
        bookHash,
        sectionIndex,
        chapterTitle,
        text: chunkText,
        pageNumber: Math.floor((cumulativeSizeBeforeSection + position) / SIZE_PER_PAGE),
      })
      chunkIndex++
    }
    position = Math.max(chunkEnd - opts.overlapSize, position + 1)
  }
  return chunks
}
