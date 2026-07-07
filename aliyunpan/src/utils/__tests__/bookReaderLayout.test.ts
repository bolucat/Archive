import { describe, expect, it } from 'vitest'
import { applyReaderBrightnessToColor, buildReaderContentMarginCss, buildReaderStageStyle } from '../bookReaderLayout'

describe('buildReaderStageStyle', () => {
  it('shrinks container width when margin increases (koodo approach)', () => {
    // Node env: vw = 1200
    // margin=0: rawWidth = 1200*1 - 1200*0.4 - 0 = 720
    expect(buildReaderStageStyle({ scale: 1, margin: 0, backgroundColor: '#fff', textColor: '#111', brightness: 1 }).width).toBe('min(720px, calc(100% - 40px))')
    // margin=40: rawWidth = 1200*1 - 1200*0.4 - 80 = 640
    expect(buildReaderStageStyle({ scale: 1, margin: 40, backgroundColor: '#fff', textColor: '#111', brightness: 1 }).width).toBe('min(640px, calc(100% - 40px))')
  })

  it('builds content margin CSS (no body padding — koodo uses container width for margin)', () => {
    // koodo 方式：margin 通过缩小容器宽度实现，body 不加 padding
    const css = buildReaderContentMarginCss(40)
    expect(css).not.toContain('padding-left')
    expect(css).not.toContain('padding-right')
    expect(css).toContain('box-sizing: border-box')
    // 单页/滚动/双页模式都不应注入 column 覆盖（让 boxplayer 自然分页/滚动）
    expect(css).not.toContain('column-count')
    const doubleCss = buildReaderContentMarginCss(40, 'double')
    expect(doubleCss).not.toContain('column-count')
    const scrollCss = buildReaderContentMarginCss(40, 'scroll')
    expect(scrollCss).not.toContain('column-count')
  })

  it('keeps the outer reader background visually aligned with brightness-filtered pages', () => {
    expect(applyReaderBrightnessToColor('rgba(233,223,200,1)', 0.7)).toBe('rgba(163,156,140,1)')
    expect(applyReaderBrightnessToColor('#e9dfc8', 0.7)).toBe('#a39c8c')
  })
})
