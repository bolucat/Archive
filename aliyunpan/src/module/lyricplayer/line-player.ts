/**
 * LinePlayer — LRC lyric scheduler.
 * Parses LRC text, manages line timing, fires onPlay callback.
 */
import { getNow, TimeoutTools } from './utils'

const timeFieldExp = /^(?:\[[\d:.]+\])+/g
const timeExp = /\d{1,3}(:\d{1,3}){0,2}(?:\.\d{1,3})/g

const tagRegMap: Record<string, string> = {
  title: 'ti',
  artist: 'ar',
  album: 'al',
  offset: 'offset',
  by: 'by',
}

export interface LyricLine {
  time: number
  text: string
  extendedLyrics: string[]
}

const t_rxp_1 = /^0+(\d+)/
const t_rxp_2 = /:0+(\d+)/g
const t_rxp_3 = /\.0+(\d+)/

function formatTimeLabel(label: string): string {
  return label
    .replace(t_rxp_1, '$1')
    .replace(t_rxp_2, ':$1')
    .replace(t_rxp_3, '.$1')
}

function parseExtendedLyric(
  lrcLinesMap: Record<string, LyricLine>,
  extendedLyric: string
) {
  const lines = extendedLyric.split(/\r\n|\n|\r/)
  for (const line of lines) {
    const trimmed = line.trim()
    const result = timeFieldExp.exec(trimmed)
    if (result) {
      const timeField = result[0]
      const text = trimmed.replace(timeFieldExp, '').trim()
      if (text && text !== '//') {
        const times = timeField.match(timeExp)
        if (!times) continue
        for (const time of times) {
          const timeStr = formatTimeLabel(time)
          const targetLine = lrcLinesMap[timeStr]
          if (targetLine) targetLine.extendedLyrics.push(text)
        }
      }
    }
  }
}

export default class LinePlayer {
  tags: Record<string, string | number> = {}
  lines: LyricLine[] = []
  lyric = ''
  extendedLyrics: string[] = []

  onPlay: (lineNum: number, text: string, curTime: number) => void
  onSetLyric: (lines: LyricLine[], offset: number) => void

  isPlay = false
  curLineNum = 0
  maxLine = 0
  offset = 0
  private _performanceTime = 0
  private _startTime = 0
  private _rate = 1

  private timeoutTools = new TimeoutTools()

  constructor({
    offset = 0,
    rate = 1,
    onPlay = () => {},
    onSetLyric = () => {},
  }: {
    offset?: number
    rate?: number
    onPlay?: (lineNum: number, text: string, curTime: number) => void
    onSetLyric?: (lines: LyricLine[], offset: number) => void
  } = {}) {
    this.onPlay = onPlay
    this.onSetLyric = onSetLyric
    this.offset = offset
    this._rate = rate
  }

  setLyric(lyric: string, extendedLyrics: string[] = []) {
    this.pause()
    this.lyric = lyric || ''
    this.extendedLyrics = extendedLyrics
    this._initTag()
    this._initLines()
    this.onSetLyric(this.lines, (this.tags.offset as number) + this.offset)
  }

  private _initTag() {
    this.tags = {}
    for (const tag in tagRegMap) {
      const matches = this.lyric.match(
        new RegExp(`\\[${tagRegMap[tag]}:([^\\]]*)]`, 'i')
      )
      this.tags[tag] = (matches && matches[1]) || ''
    }
    if (this.tags.offset) {
      const parsedOffset = parseInt(String(this.tags.offset))
      this.tags.offset = Number.isNaN(parsedOffset) ? 0 : parsedOffset
    } else {
      this.tags.offset = 0
    }
  }

  private _initLines() {
    this.lines = []
    const lines = this.lyric.split(/\r\n|\r|\n/)
    const linesMap: Record<string, LyricLine> = {}

    for (const line of lines) {
      const trimmed = line.trim()
      const result = timeFieldExp.exec(trimmed)
      if (result) {
        const timeField = result[0]
        const text = trimmed.replace(timeFieldExp, '').trim()
        if (text) {
          const times = timeField.match(timeExp)
          if (!times) continue
          for (const time of times) {
            const timeStr = formatTimeLabel(time)
            if (linesMap[timeStr]) {
              linesMap[timeStr].extendedLyrics.push(text)
              continue
            }
            const timeArr = timeStr.split(':')
            if (timeArr.length > 3) continue
            if (timeArr.length < 3) {
              while (timeArr.length < 3) timeArr.unshift('0')
            }
            if (timeArr[2].indexOf('.') > -1) {
              const [sec, ms] = timeArr[2].split('.')
              timeArr.splice(2, 1, sec, ms || '0')
            } else {
              timeArr.push('0')
            }

            linesMap[timeStr] = {
              time:
                parseInt(timeArr[0]) * 3600000 +
                parseInt(timeArr[1]) * 60000 +
                parseInt(timeArr[2]) * 1000 +
                parseInt(timeArr[3] || '0'),
              text,
              extendedLyrics: [],
            }
          }
        }
      }
    }

    for (const lrc of this.extendedLyrics) {
      parseExtendedLyric(linesMap, lrc)
    }
    this.lines = Object.values(linesMap).sort((a, b) => a.time - b.time)
    this.maxLine = this.lines.length - 1
  }

  private _currentTime() {
    return (getNow() - this._performanceTime) * this._rate + this._startTime
  }

  private _findCurLineNum(curTime: number, startIndex = 0): number {
    if (curTime <= 0) return 0
    for (let i = startIndex; i < this.lines.length; i++) {
      if (curTime <= this.lines[i].time) return i === 0 ? 0 : i - 1
    }
    return this.lines.length - 1
  }

  private _refresh() {
    this.curLineNum++
    if (this.curLineNum >= this.maxLine) {
      this.onPlay(
        this.curLineNum,
        this.lines[this.curLineNum]?.text ?? '',
        this._currentTime()
      )
      this.pause()
      return
    }
    const curLine = this.lines[this.curLineNum]
    const currentTime = this._currentTime()
    const driftTime = currentTime - curLine.time
    if (driftTime >= 0) {
      const nextLine = this.lines[this.curLineNum + 1]
      const delay = (nextLine.time - curLine.time - driftTime) / this._rate
      if (delay > 0) {
        if (this.isPlay) {
          this.timeoutTools.start(() => {
            if (!this.isPlay) return
            this._refresh()
          }, delay)
        }
        this.onPlay(this.curLineNum, curLine.text, currentTime)
      } else {
        const newNum = this._findCurLineNum(currentTime, this.curLineNum + 1)
        if (newNum > this.curLineNum) this.curLineNum = newNum - 1
        this._refresh()
      }
    } else if (this.curLineNum === 0) {
      const firstLine = this.lines[0]
      const delay = (firstLine.time - currentTime) / this._rate
      if (this.isPlay) {
        this.timeoutTools.start(() => {
          if (!this.isPlay) return
          this._refresh()
        }, delay)
      }
      this.onPlay(-1, '', currentTime)
    } else {
      this.curLineNum = this._findCurLineNum(currentTime, this.curLineNum) - 1
      this._refresh()
    }
  }

  play(curTime = 0) {
    if (!this.lines.length) return
    this.pause()
    this.isPlay = true
    this._performanceTime = getNow() - ((this.tags.offset as number) + this.offset)
    this._startTime = curTime
    this.curLineNum = this._findCurLineNum(this._currentTime()) - 1
    this._refresh()
  }

  pause() {
    if (!this.isPlay) return
    this.isPlay = false
    this.timeoutTools.clear()
    if (this.curLineNum === this.maxLine) return
    const currentTime = this._currentTime()
    const curLineNum = this._findCurLineNum(currentTime)
    if (this.curLineNum !== curLineNum) {
      this.curLineNum = curLineNum
      this.onPlay(curLineNum, this.lines[curLineNum]?.text ?? '', currentTime)
    }
  }

  setPlaybackRate(rate: number) {
    this._rate = rate
    if (!this.lines.length) return
    if (!this.isPlay) return
    this.play(this._currentTime())
  }
}
