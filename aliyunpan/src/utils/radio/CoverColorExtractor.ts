/**
 * CoverColorExtractor — extract dominant colors from album cover images.
 * Used for automatic visual tint, lyric coloring, and background theming.
 * Ported from Mineradio's hexToRgb / rgbToHsl / hslToRgb + cover sampling.
 */

export interface CoverPalette {
  /** Hex primary color from cover. */
  primary: string
  /** Hex secondary / complementary. */
  secondary: string
  /** Hex highlight color. */
  highlight: string
  /** Array of 5 swatch hex colors sorted by dominance. */
  swatches: string[]
}

export function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace('#', '')
  return [
    parseInt(h.slice(0, 2), 16) / 255,
    parseInt(h.slice(2, 4), 16) / 255,
    parseInt(h.slice(4, 6), 16) / 255,
  ]
}

export function rgbToHex(r: number, g: number, b: number): string {
  const toHex = (v: number) => Math.max(0, Math.min(255, Math.round(v * 255)))
    .toString(16).padStart(2, '0')
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`
}

export function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  const l = (max + min) / 2
  if (max === min) return [0, 0, l]
  const d = max - min
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
  let h = 0
  if (max === r) h = ((g - b) / d + (g < b ? 6 : 0)) / 6
  else if (max === g) h = ((b - r) / d + 2) / 6
  else h = ((r - g) / d + 4) / 6
  return [h, s, l]
}

export function hslToRgb(h: number, s: number, l: number): [number, number, number] {
  const hue2rgb = (p: number, q: number, t: number) => {
    if (t < 0) t += 1
    if (t > 1) t -= 1
    if (t < 1 / 6) return p + (q - p) * 6 * t
    if (t < 1 / 2) return q
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6
    return p
  }
  if (s === 0) return [l, l, l]
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s
  const p = 2 * l - q
  return [hue2rgb(p, q, h + 1 / 3), hue2rgb(p, q, h), hue2rgb(p, q, h - 1 / 3)]
}

/** Extract a CoverPalette from an image element. Returns null if extraction fails. */
export function extractCoverPalette(img: HTMLImageElement | HTMLCanvasElement, swatchCount = 5): CoverPalette | null {
  try {
    const canvas = document.createElement('canvas')
    const size = 80
    canvas.width = size
    canvas.height = size
    const ctx = canvas.getContext('2d')
    if (!ctx) return null
    ctx.drawImage(img, 0, 0, size, size)

    const data = ctx.getImageData(0, 0, size, size).data
    const colorMap = new Map<string, number>()

    for (let i = 0; i < data.length; i += 4) {
      const r = data[i]
      const g = data[i + 1]
      const b = data[i + 2]
      const a = data[i + 3]
      if (a < 64) continue
      // Quantize to 16 levels per channel for grouping
      const qr = Math.round(r / 16) * 16
      const qg = Math.round(g / 16) * 16
      const qb = Math.round(b / 16) * 16
      const key = `${qr},${qg},${qb}`
      colorMap.set(key, (colorMap.get(key) || 0) + 1)
    }

    const sorted = [...colorMap.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, swatchCount)

    const swatches = sorted.map(([key]) => {
      const [r, g, b] = key.split(',').map(Number)
      return rgbToHex(r / 255, g / 255, b / 255)
    })

    const primary = swatches[0] || '#9db8cf'
    const [pr, pg, pb] = hexToRgb(primary)
    const [h, s, l] = rgbToHsl(pr, pg, pb)

    // Secondary: complementary hue
    const [sr, sg, sb] = hslToRgb((h + 0.5) % 1, Math.min(1, s * 0.7), l)
    const secondary = rgbToHex(sr, sg, sb)

    // Highlight: lighter version
    const [hr, hg, hb] = hslToRgb(h, Math.min(1, s * 0.5), Math.min(1, l + 0.15))
    const highlight = rgbToHex(hr, hg, hb)

    return { primary, secondary, highlight, swatches }
  } catch {
    return null
  }
}
