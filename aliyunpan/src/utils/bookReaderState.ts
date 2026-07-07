import type { IBookItem } from '../types/book'

export interface BookReaderPosition {
  percentage?: number
  chapterTitle?: string
  chapterDocIndex?: string
  chapterHref?: string
  count?: string | number
  cfi?: string
  page?: string
  text?: string
  xpath?: string
  [key: string]: unknown
}

export function normalizeReaderPosition(position: any): BookReaderPosition {
  const normalized: BookReaderPosition = {}
  if (!position || typeof position !== 'object') return normalized
  for (const [key, value] of Object.entries(position)) {
    if (value === undefined || value === null) continue
    if (key === 'percentage') {
      const numeric = Number(value)
      if (Number.isFinite(numeric)) normalized.percentage = numeric
    } else if (['chapterTitle', 'chapterDocIndex', 'chapterHref', 'cfi', 'page', 'text', 'xpath'].includes(key)) {
      normalized[key] = String(value)
    } else if (key === 'count') {
      normalized.count = typeof value === 'number' ? value : String(value)
    }
  }
  return normalized
}

export function getBookReadingProgressText(position?: BookReaderPosition): string {
  if (!position) return ''
  if (position.percentage !== undefined) {
    return `${Math.max(0, Math.min(100, Math.round(position.percentage * 100)))}%`
  }
  if (position.page) return position.page
  if (position.chapterTitle) return position.chapterTitle
  return ''
}

export function buildBookReadingPatch(
  position: BookReaderPosition,
  now = Date.now()
): Partial<IBookItem> {
  return {
    reading_position: position,
    reading_progress: position.percentage !== undefined
      ? Math.max(0, Math.min(100, Math.round(position.percentage * 100)))
      : undefined,
    reading_progress_text: getBookReadingProgressText(position),
    reading_chapter: position.chapterTitle || '',
    last_read_at: now
  }
}

export function buildBookReadingTimePatch(
  currentReadingTime: unknown,
  sessionStartedAt: number,
  now = Date.now()
): Partial<IBookItem> {
  const elapsedSeconds = Math.floor((now - sessionStartedAt) / 1000)
  if (!sessionStartedAt || elapsedSeconds <= 0) return {}
  const current = Number(currentReadingTime)
  return {
    reading_time: Math.max(0, Number.isFinite(current) ? current : 0) + elapsedSeconds,
    last_read_at: now
  }
}
