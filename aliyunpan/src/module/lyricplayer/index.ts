/**
 * LyricPlayer — Vue-friendly wrapper around LinePlayer.
 * Parses LRC text and tracks the active lyric line index.
 */
import LinePlayer, { type LyricLine } from './line-player'
import { getNow } from './utils'

export interface LyricPlayerInstance {
  /** Current lyric lines indexed by their position */
  lines: LyricLine[]
  /** Index of the currently active line (-1 if none) */
  activeIndex: number
  /** Update the player with current audio time in seconds */
  play: (currentTimeSec: number) => void
  /** Pause lyric tracking */
  pause: () => void
  /** Set new lyric text */
  setLyric: (lrc: string, extendedLyrics?: string[]) => LyricLine[]
  /** Change offset (milliseconds) */
  setOffset: (offsetMs: number) => void
  /** Clean up */
  destroy: () => void
}

export function createLyricPlayer(): LyricPlayerInstance {
  const player: LyricPlayerInstance = {
    lines: [],
    activeIndex: -1,
    play: () => {},
    pause: () => {},
    setLyric: () => [],
    setOffset: () => {},
    destroy: () => {},
  }

  const linePlayer = new LinePlayer({
    onPlay(lineNum, _text) {
      player.activeIndex = lineNum
    },
    onSetLyric(lines) {
      player.lines = lines
    },
  })

  player.play = (currentTimeSec: number) => {
    linePlayer.play(currentTimeSec * 1000)
  }

  player.pause = () => {
    linePlayer.pause()
  }

  player.setLyric = (lrc: string, extendedLyrics?: string[]) => {
    linePlayer.setLyric(lrc, extendedLyrics || [])
    return linePlayer.lines
  }

  player.setOffset = (offsetMs: number) => {
    linePlayer.offset = offsetMs
    linePlayer.setLyric(linePlayer.lyric, linePlayer.extendedLyrics)
  }

  player.destroy = () => {
    linePlayer.pause()
  }

  return player
}

// ---- Helpers for creating per-word timed DOM ----

export function createTimedLyricDOM(
  lrcText: string,
  options: {
    lineContentClassName?: string
    lineClassName?: string
    shadowClassName?: string
    fontLrcClassName?: string
    extendedLrcClassName?: string
    shadowContent?: boolean
  } = {}
): Map<number, HTMLElement> {
  const {
    lineContentClassName = 'lyric-line',
    lineClassName = 'lyric-line-inner',
    shadowClassName = 'lyric-shadow',
    fontLrcClassName = 'lyric-text',
    extendedLrcClassName = 'lyric-extended',
    shadowContent = true,
  } = options

  const lines = parseLrcIntoLines(lrcText)
  const result = new Map<number, HTMLElement>()

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    const content = document.createElement('div')
    content.className = lineContentClassName
    ;(content as any).__time = line.time

    const inner = document.createElement('div')
    inner.className = lineClassName
    inner.style.cssText = 'position:relative;display:inline-block'
    content.appendChild(inner)

    const textEl = document.createElement('div')
    textEl.className = fontLrcClassName

    const fonts = splitFonts(line.text)
    if (fonts) {
      // Per-word timed
      for (const font of fonts) {
        const span = document.createElement('span')
        span.textContent = font.text
        textEl.appendChild(span)
      }
      if (shadowContent) {
        const shadow = document.createElement('div')
        shadow.className = shadowClassName
        shadow.style.cssText = 'position:absolute;top:0;left:0;right:0;z-index:-1'
        for (const font of fonts) {
          const s = document.createElement('span')
          s.textContent = font.text
          shadow.appendChild(s)
        }
        inner.appendChild(shadow)
      }
    } else {
      // Plain text
      textEl.textContent = line.text
    }

    inner.appendChild(textEl)
    content.appendChild(inner)

    for (const ext of line.extendedLyrics) {
      content.appendChild(document.createElement('br'))
      const extEl = document.createElement('div')
      extEl.className = extendedLrcClassName
      extEl.textContent = ext
      content.appendChild(extEl)
    }

    result.set(i, content)
  }

  return result
}

const fontSplitRxp = /(?=<\d+,\d+>).*?/g
const timeRxp = /^<\d+,\d+>/

interface TimedFont {
  text: string
  startTime: number
  duration: number
}

function splitFonts(text: string): TimedFont[] | null {
  const parts = text.split(fontSplitRxp)
  if (parts.length <= 1) return null
  const fonts: TimedFont[] = []
  for (const part of parts) {
    if (!timeRxp.test(part)) continue
    const match = part.match(/^<(\d+),(\d+)>/)
    if (!match) continue
    fonts.push({
      text: part.replace(timeRxp, ''),
      startTime: parseInt(match[1]),
      duration: parseInt(match[2]),
    })
  }
  return fonts.length > 0 ? fonts : null
}

interface ParsedLyricLine {
  time: number
  text: string
  extendedLyrics: string[]
}

function parseLrcIntoLines(lrc: string): ParsedLyricLine[] {
  const lines: ParsedLyricLine[] = []
  const rawLines = lrc.split(/\r?\n/)
  for (const line of rawLines) {
    const match = /^\[(\d{1,3}):(\d{1,3}([.:]\d{1,3})?)\]/.exec(line.trim())
    if (!match) continue
    const mins = parseInt(match[1])
    const secs = parseFloat(match[2].replace(':', '.'))
    const text = line.trim().replace(/^\[[\d:.]+\]\s*/, '')
    if (!text) continue
    lines.push({
      time: mins * 60 + secs,
      text,
      extendedLyrics: [],
    })
  }
  return lines
}

export type { LyricLine }
