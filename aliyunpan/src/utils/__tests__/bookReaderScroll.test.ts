import { describe, expect, it, vi } from 'vitest'
import { scrollReaderPageArea } from '../bookReaderScroll'

function createScrollElement(values: { scrollTop: number; scrollHeight: number; clientHeight: number }) {
  return {
    ...values,
    scrollTo: vi.fn((options: ScrollToOptions) => {
      if (typeof options.top === 'number') values.scrollTop = options.top
    }),
    get scrollTop() {
      return values.scrollTop
    },
    set scrollTop(value: number) {
      values.scrollTop = value
    },
    get scrollHeight() {
      return values.scrollHeight
    },
    get clientHeight() {
      return values.clientHeight
    }
  } as unknown as HTMLElement
}

describe('book reader scroll mode navigation', () => {
  it('scrolls the page area down when the next button is clicked within a chapter', async () => {
    const element = createScrollElement({ scrollTop: 100, scrollHeight: 2000, clientHeight: 500 })
    const reader = { prev: vi.fn(), next: vi.fn() }

    const result = await scrollReaderPageArea(element, reader, 1)

    expect(result).toBe('scrolled')
    expect(element.scrollTop).toBe(510)
    expect(reader.next).not.toHaveBeenCalled()
  })

  it('scrolls the page area up when the previous button is clicked within a chapter', async () => {
    const element = createScrollElement({ scrollTop: 600, scrollHeight: 2000, clientHeight: 500 })
    const reader = { prev: vi.fn(), next: vi.fn() }

    const result = await scrollReaderPageArea(element, reader, -1)

    expect(result).toBe('scrolled')
    expect(element.scrollTop).toBe(190)
    expect(reader.prev).not.toHaveBeenCalled()
  })

  it('moves to the adjacent chapter when scroll navigation reaches an edge', async () => {
    const atTop = createScrollElement({ scrollTop: 0, scrollHeight: 2000, clientHeight: 500 })
    const atBottom = createScrollElement({ scrollTop: 1495, scrollHeight: 2000, clientHeight: 500 })
    const reader = { prev: vi.fn(), next: vi.fn() }

    expect(await scrollReaderPageArea(atTop, reader, -1)).toBe('previous-chapter')
    expect(await scrollReaderPageArea(atBottom, reader, 1)).toBe('next-chapter')
    expect(reader.prev).toHaveBeenCalledTimes(1)
    expect(reader.next).toHaveBeenCalledTimes(1)
  })

  it('moves to the next chapter when the remaining scroll distance is less than one page step', async () => {
    const shortRemainingPage = createScrollElement({ scrollTop: 0, scrollHeight: 842, clientHeight: 819 })
    const reader = { prev: vi.fn(), next: vi.fn() }

    expect(await scrollReaderPageArea(shortRemainingPage, reader, 1)).toBe('next-chapter')
    expect(shortRemainingPage.scrollTop).toBe(0)
    expect(reader.next).toHaveBeenCalledTimes(1)
  })
})
