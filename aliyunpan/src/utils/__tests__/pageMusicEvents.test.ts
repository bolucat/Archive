import { describe, expect, it } from 'vitest'
import { shouldHandleMusicPlayerDblClick } from '../pageMusicEvents'

function eventFromClassName(className: string) {
  const target = {
    closest(selector: string) {
      return selector
        .split(',')
        .map((s) => s.trim().replace(/^\./, ''))
        .some((cls) => className.split(/\s+/).includes(cls))
        ? target
        : null
    }
  }
  return { target } as unknown as MouseEvent
}

describe('pageMusicEvents', () => {
  it('ignores double clicks from the lyrics toolbar button', () => {
    expect(shouldHandleMusicPlayerDblClick(eventFromClassName('amp-icon-btn active'))).toBe(false)
  })

  it('allows double clicks from the passive player surface', () => {
    expect(shouldHandleMusicPlayerDblClick(eventFromClassName('amp-stage-cover'))).toBe(true)
  })
})
