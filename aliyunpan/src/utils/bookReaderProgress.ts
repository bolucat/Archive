import type { BookReaderPosition } from './bookReaderState'

export interface ReaderPageProgress {
  currentPage: number
  totalPage: number
  percentage?: number
}

function finiteNumber(value: unknown): number | undefined {
  const numeric = Number(value)
  return Number.isFinite(numeric) ? numeric : undefined
}

function positiveInteger(value: unknown): number {
  const numeric = finiteNumber(value)
  return numeric && numeric > 0 ? Math.floor(numeric) : 0
}

function parsePagePair(value: unknown): Pick<ReaderPageProgress, 'currentPage' | 'totalPage'> {
  if (typeof value !== 'string') return { currentPage: positiveInteger(value), totalPage: 0 }
  const match = value.match(/(\d+)\s*(?:\/|of)\s*(\d+)/i)
  if (match) {
    return {
      currentPage: positiveInteger(match[1]),
      totalPage: positiveInteger(match[2])
    }
  }
  return { currentPage: positiveInteger(value), totalPage: 0 }
}

export function normalizeReaderPageProgress(progress: any, position?: BookReaderPosition): ReaderPageProgress {
  const positionPage = parsePagePair(position?.page)
  const progressPage = parsePagePair(progress?.currentPage ?? progress?.page ?? progress?.pageIndex)
  const currentPage = progressPage.currentPage || positionPage.currentPage
  const totalPage = positiveInteger(progress?.totalPage ?? progress?.totalPages ?? progress?.pageCount ?? progress?.total) || positionPage.totalPage
  const percentage = finiteNumber(progress?.percentage ?? position?.percentage)

  return {
    currentPage,
    totalPage,
    percentage
  }
}

export function estimateReaderPageProgressFromElement(element: Pick<HTMLElement, 'clientHeight' | 'scrollHeight' | 'scrollTop'> | null | undefined): ReaderPageProgress {
  if (!element?.clientHeight) return { currentPage: 0, totalPage: 0 }
  const totalPage = Math.max(1, Math.ceil(element.scrollHeight / element.clientHeight))
  const currentPage = Math.max(1, Math.min(totalPage, Math.floor(element.scrollTop / element.clientHeight) + 1))
  return { currentPage, totalPage }
}
