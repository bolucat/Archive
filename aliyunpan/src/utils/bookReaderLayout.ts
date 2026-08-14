export interface ReaderStageStyleOptions {
  scale: number
  margin: number
  backgroundColor: string
  textColor: string
  brightness: number
  invert?: boolean
}

function clampColor(value: number): number {
  return Math.max(0, Math.min(255, Math.round(value)))
}

export function applyReaderBrightnessToColor(color: string, brightness: number): string {
  const amount = Number.isFinite(brightness) ? brightness : 1
  const rgba = color.match(/rgba?\(\s*(\d+),\s*(\d+),\s*(\d+)(?:,\s*([.\d]+))?\s*\)/i)
  if (rgba) {
    const alpha = rgba[4] ?? (color.trim().toLowerCase().startsWith('rgba') ? '1' : undefined)
    const rgb = [rgba[1], rgba[2], rgba[3]].map((part) => clampColor(Number(part) * amount))
    return alpha === undefined ? `rgb(${rgb[0]},${rgb[1]},${rgb[2]})` : `rgba(${rgb[0]},${rgb[1]},${rgb[2]},${alpha})`
  }

  const hex = color.match(/^#([0-9a-f]{6})$/i)
  if (hex) {
    const value = hex[1]
    const rgb = [value.slice(0, 2), value.slice(2, 4), value.slice(4, 6)].map((part) => clampColor(parseInt(part, 16) * amount).toString(16).padStart(2, '0'))
    return `#${rgb.join('')}`
  }

  return color
}

export function buildReaderStageStyle(options: ReaderStageStyleOptions & { layoutMode?: string }) {
  const margin = Math.max(24, Number.isFinite(options.margin) ? options.margin : 24)
  // Keep a readable minimum measure on narrow windows. This also prevents a
  // large margin from shrinking a paginated iframe enough to expose adjacent pages.
  const width = `min(calc(100% - 48px), max(360px, calc(100% - ${margin * 2}px)))`

  const filter = `brightness(${options.brightness})${options.invert ? ' invert(1) hue-rotate(180deg)' : ''}`

  if (options.layoutMode === 'scroll') {
    // koodo: margin 缩小容器宽度而非给 body 加 padding
    return {
      width,
      margin: '0 auto',
      backgroundColor: options.backgroundColor,
      color: options.textColor,
      filter
    }
  }

  if (options.layoutMode === 'double') {
    return {
      width,
      margin: '0 auto',
      backgroundColor: options.backgroundColor,
      color: options.textColor,
      filter
    }
  }

  // 单页/双页/滚动统一以页边距确定正文距离左右边界的留白。
  return {
    width,
    margin: '0 auto',
    backgroundColor: options.backgroundColor,
    color: options.textColor,
    filter
  }
}

export function buildReaderContentMarginCss(margin: number, layoutMode?: string): string {
  // koodo 方式：margin 已通过缩小容器宽度处理，body 不再加 padding
  // 单页/滚动模式：完全交给 kookit 引擎处理 (column-fill:auto + column-width
  // 用于分页；scrolled-doc 模式不设栏)，外部不要再注入 column-count 覆盖，
  // 否则会强行把内容压成 1 栏并禁用 kookit 的横向分页/容器滚动。
  // 双页模式：column-count 由 applyDoublePageCss 在 documentElement 上设置。
  void margin
  void layoutMode
  return `
html, body {
  box-sizing: border-box!important;
}
body * {
  box-sizing: border-box;
}
`
}
