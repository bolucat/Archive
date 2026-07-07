import { describe, expect, it } from 'vitest'
import { buildReferPopupPosition, sanitizeReferHtml, stripReferHtml } from '../bookRefer'

describe('readerContainerer', () => {
  it('strips footnote html for copy action', () => {
    expect(stripReferHtml('<p>Note <b>one</b></p><p>Line&nbsp;two &amp; three</p>')).toBe('Note one Line two & three')
  })

  it('sanitizes scriptable footnote html before rendering', () => {
    expect(sanitizeReferHtml('<p onclick="run()">Note</p><img src="javascript:run()" onerror="run()"><script>run()</script>'))
      .toBe('<p>Note</p><img src="#">')
  })

  it('positions the refer popup near the clicked link and clamps to viewport', () => {
    expect(buildReferPopupPosition(
      { left: 240, top: 300, width: 40, height: 16, bottom: 316 },
      { left: 100, top: 50 },
      { width: 800, height: 600 }
    )).toEqual({ x: 215, y: 330 })

    expect(buildReferPopupPosition(
      { left: 760, top: 560, width: 80, height: 16, bottom: 576 },
      { left: 0, top: 0 },
      { width: 800, height: 600 }
    )).toEqual({ x: 490, y: 330 })
  })
})
