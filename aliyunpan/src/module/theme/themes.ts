// Ported from lx-music-desktop common/theme/colorUtils.js
// https://github.com/PimpTrizkit/PJs/wiki/12.-Shade,-Blend-and-Convert-a-Web-Color-(pSBC.js)

function RGB_Linear_Shade(p: number, c0: string): string {
  const i = parseInt, r = Math.round
  const [a, b, c, d] = c0.split(',')
  const n = p < 0
  const t = n ? 0 : 255 * p
  const P = n ? 1 + p : 1 - p
  return 'rgb' + (d ? 'a(' : '(') +
    r(i(a[3] === 'a' ? a.slice(5) : a.slice(4)) * P + t) + ',' +
    r(i(b) * P + t) + ',' +
    r(i(c) * P + t) +
    (d ? ',' + d : ')')
}

function RGB_Alpha_Shade(p: number, color: string): string {
  const i = parseInt
  const n = p < 0
  let [r, g, b, a] = color.split(',')
  r = r[3] === 'a' ? r.slice(5) : r.slice(4)
  if (a) {
    let alpha = parseFloat(a)
    alpha = alpha - (n ? (1 - alpha) * p : alpha * p)
    a = String(n ? Math.max(0, alpha) : Math.min(1, alpha))
  } else {
    a = String(Math.min(1, 1 - p))
  }
  return `rgba(${i(r)}, ${i(g)}, ${i(b)}, ${Number(a).toFixed(2)})`
}

function createFontColors(rgbaColor: string, isDark: boolean): Record<string, string> {
  rgbaColor ??= isDark ? 'rgb(229, 229, 229)' : 'rgb(33, 33, 33)'
  if (isDark) return createFontDarkColors(rgbaColor)
  const colors: Record<string, string> = { '--color-1000': rgbaColor }
  const step = 0.05
  for (let i = 1; i < 21; i++) {
    colors[`--color-${String(1000 - 50 * i).padStart(3, '0')}`] = RGB_Linear_Shade(step * i, rgbaColor)
  }
  return colors
}

function createFontDarkColors(rgbaColor: string): Record<string, string> {
  const colors: Record<string, string> = { '--color-1000': rgbaColor }
  let preColor = rgbaColor
  for (let i = 1; i < 21; i++) {
    preColor = RGB_Linear_Shade(-0.05, preColor)
    colors[`--color-${String(1000 - 50 * i).padStart(3, '0')}`] = preColor
  }
  return colors
}

export function createThemeColors(
  rgbaColor: string,
  fontRgbaColor: string | null,
  isDark: boolean,
  isDarkFont: boolean
): Record<string, string> {
  const colors: Record<string, string> = {
    '--color-primary': rgbaColor,
  }

  let preColor = rgbaColor
  for (let i = 1; i < 11; i++) {
    preColor = RGB_Linear_Shade(isDark ? 0.2 : -0.1, preColor)
    colors[`--color-primary-dark-${i * 100}`] = preColor
    for (let j = 1; j < 10; j++) {
      colors[`--color-primary-dark-${i * 100}-alpha-${j * 100}`] = RGB_Alpha_Shade(0.1 * j, preColor)
      colors[`--color-primary-alpha-${j * 100}`] = RGB_Alpha_Shade(0.1 * j, rgbaColor)
    }
  }

  preColor = rgbaColor
  for (let i = 1; i < 10; i++) {
    preColor = RGB_Linear_Shade(isDark ? -0.1 : 0.2, preColor)
    colors[`--color-primary-light-${i * 100}`] = preColor
    for (let j = 1; j < 10; j++) {
      colors[`--color-primary-light-${i * 100}-alpha-${j * 100}`] = RGB_Alpha_Shade(0.1 * j, preColor)
    }
  }
  preColor = RGB_Linear_Shade(isDark ? -0.35 : 1, preColor)
  colors[`--color-primary-light-${1000}`] = preColor
  for (let j = 1; j < 10; j++) {
    colors[`--color-primary-light-${1000}-alpha-${j * 100}`] = RGB_Alpha_Shade(0.1 * j, preColor)
  }

  colors['--color-theme'] = isDark ? colors['--color-primary-light-900'] : rgbaColor

  const fontColors = createFontColors(fontRgbaColor ?? '', isDark)
  return { ...colors, ...fontColors }
}

export interface ThemeDef {
  id: string
  name: string
  isDark: boolean
  primaryColor: string
  fontColor: string
}

export const defaultThemes: ThemeDef[] = [
  { id: 'green', name: '绿色', isDark: true, primaryColor: 'rgb(34, 173, 56)', fontColor: 'rgb(229, 229, 229)' },
  { id: 'blue', name: '蓝色', isDark: true, primaryColor: 'rgb(52, 119, 235)', fontColor: 'rgb(229, 229, 229)' },
  { id: 'purple', name: '紫色', isDark: true, primaryColor: 'rgb(139, 69, 225)', fontColor: 'rgb(229, 229, 229)' },
  { id: 'orange', name: '橙色', isDark: true, primaryColor: 'rgb(255, 136, 0)', fontColor: 'rgb(229, 229, 229)' },
  { id: 'red', name: '红色', isDark: true, primaryColor: 'rgb(229, 62, 62)', fontColor: 'rgb(229, 229, 229)' },
  { id: 'pink', name: '粉色', isDark: true, primaryColor: 'rgb(236, 75, 140)', fontColor: 'rgb(229, 229, 229)' },
  { id: 'grey', name: '灰色', isDark: true, primaryColor: 'rgb(130, 130, 140)', fontColor: 'rgb(229, 229, 229)' },
  { id: 'light_blue', name: '天蓝', isDark: false, primaryColor: 'rgb(52, 119, 235)', fontColor: 'rgb(33, 33, 33)' },
  { id: 'light_green', name: '浅绿', isDark: false, primaryColor: 'rgb(34, 173, 56)', fontColor: 'rgb(33, 33, 33)' },
  { id: 'light_purple', name: '浅紫', isDark: false, primaryColor: 'rgb(139, 69, 225)', fontColor: 'rgb(33, 33, 33)' },
]

export function applyTheme(themeId: string, target: HTMLElement = document.documentElement) {
  const theme = defaultThemes.find(t => t.id === themeId)
  if (!theme) {
    const stored = loadCustomTheme(themeId)
    if (stored) {
      const colors = createThemeColors(stored.primaryColor, stored.fontColor || null, stored.isDark, false)
      for (const [key, val] of Object.entries(colors)) {
        target.style.setProperty(key, val)
      }
    }
    return
  }
  const colors = createThemeColors(theme.primaryColor, theme.fontColor || null, theme.isDark, false)
  for (const [key, val] of Object.entries(colors)) {
    target.style.setProperty(key, val)
  }
  target.setAttribute('data-theme', themeId)
}

export function loadCustomTheme(id: string): { primaryColor: string; fontColor: string | null; isDark: boolean } | null {
  try {
    const stored = localStorage.getItem(`theme_custom_${id}`)
    if (!stored) return null
    return JSON.parse(stored)
  } catch {
    return null
  }
}

export function saveCustomTheme(id: string, data: { primaryColor: string; fontColor: string | null; isDark: boolean }) {
  localStorage.setItem(`theme_custom_${id}`, JSON.stringify(data))
}
