import { describe, expect, it } from 'vitest'
import { shouldHidePanelOnMouseLeave } from '../bookReaderParity'

describe('reader panel visibility', () => {
  it('does not auto-hide a side panel while its resize handle is being dragged', () => {
    expect(shouldHidePanelOnMouseLeave('left', false, 'left')).toBe(false)
    expect(shouldHidePanelOnMouseLeave('right', false, 'right')).toBe(false)
    expect(shouldHidePanelOnMouseLeave('left', false, 'right')).toBe(true)
    expect(shouldHidePanelOnMouseLeave('left', true, null)).toBe(false)
  })
})
