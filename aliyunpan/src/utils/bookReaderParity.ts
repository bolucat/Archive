import type { BookReaderPosition } from './bookReaderState'

export function getPanelVisible(open: boolean, locked: boolean): boolean {
  return open || locked
}

export function nextPanelLockState(currentlyLocked: boolean): { locked: boolean; open: boolean } {
  return currentlyLocked ? { locked: false, open: false } : { locked: true, open: true }
}

export function shouldHidePanelOnMouseLeave(
  side: 'top' | 'left' | 'right' | 'bottom',
  locked: boolean,
  resizingSide: 'left' | 'right' | null
): boolean {
  if (locked) return false
  if ((side === 'left' || side === 'right') && resizingSide === side) return false
  return true
}

export function serializeReaderPositionForJump(position: BookReaderPosition): string {
  return JSON.stringify({
    ...position,
    count: position.cfi ? 'ignore' : position.count,
    isFirst: true
  })
}

export function buildInitialReaderPosition(position?: BookReaderPosition): BookReaderPosition {
  if (position && Object.keys(position).length > 0) {
    return {
      ...position,
      count: position.count || 0
    }
  }
  return {
    text: '',
    chapterTitle: '',
    page: '',
    chapterDocIndex: '0',
    chapterHref: '',
    count: 0,
    cfi: ''
  }
}

export function normalizeReaderPercentage(value: unknown): number {
  const numeric = Number(value)
  if (!Number.isFinite(numeric)) return 0
  return Math.max(0, Math.min(1, numeric))
}
