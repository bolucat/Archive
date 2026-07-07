export type ReaderScrollDirection = -1 | 1
export type ReaderScrollResult = 'scrolled' | 'previous-chapter' | 'next-chapter' | 'noop'

export interface ReaderScrollNavigator {
  prev: () => Promise<void> | void
  next: () => Promise<void> | void
}

const EDGE_THRESHOLD = 10
const PAGE_STEP_RATIO = 0.82

export async function scrollReaderPageArea(element: HTMLElement | null | undefined, reader: ReaderScrollNavigator | null | undefined, direction: ReaderScrollDirection): Promise<ReaderScrollResult> {
  if (!element || !reader) return 'noop'

  const clientHeight = element.clientHeight
  const scrollHeight = element.scrollHeight
  const maxScrollTop = Math.max(0, scrollHeight - clientHeight)

  if (maxScrollTop <= EDGE_THRESHOLD) return 'noop'

  if (direction < 0 && element.scrollTop <= EDGE_THRESHOLD) {
    await reader.prev()
    return 'previous-chapter'
  }

  if (direction > 0 && Math.abs(element.scrollTop + clientHeight - scrollHeight) <= EDGE_THRESHOLD) {
    await reader.next()
    return 'next-chapter'
  }

  const delta = direction * clientHeight * PAGE_STEP_RATIO
  if (direction > 0 && maxScrollTop - element.scrollTop <= Math.abs(delta) + EDGE_THRESHOLD) {
    await reader.next()
    return 'next-chapter'
  }
  if (direction < 0 && element.scrollTop <= Math.abs(delta) + EDGE_THRESHOLD) {
    await reader.prev()
    return 'previous-chapter'
  }
  const nextTop = Math.max(0, Math.min(maxScrollTop, element.scrollTop + delta))
  if (Math.abs(nextTop - element.scrollTop) < 1) return 'noop'

  element.scrollTo?.({ top: nextTop, behavior: 'smooth' })
  element.scrollTop = nextTop
  return 'scrolled'
}
