import { kebabCase, camelCase, isEmpty, pick, omitBy, isPlainObject } from 'lodash'
import { userKeys, systemKeys, needRestartKeys } from '@shared/configKeys'
import {
  APP_THEME, GRAPHIC, NONE_SELECTED_FILES, SELECTED_ALL_FILES,
  UNKNOWN_PEERID, UNKNOWN_PEERID_NAME, RESOURCE_TAGS,
  IMAGE_SUFFIXES, AUDIO_SUFFIXES, VIDEO_SUFFIXES, SUB_SUFFIXES, DOCUMENT_SUFFIXES,
  SUPPORT_RTL_LOCALES
} from '@shared/constants'

export const bytesToSize = (bytes: number, precision = 1): string => {
  if (!bytes || bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB']
  const idx = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, idx)).toFixed(precision)} ${units[idx]}`
}

export const extractSpeedUnit = (speed: string): string => {
  const m = (speed || '').match(/[A-Z]+/)
  return m ? m[0] : ''
}

export const calcProgress = (totalLength: number, completedLength: number, decimal = 2): number => {
  if (!totalLength || totalLength === 0) return 0
  const p = completedLength / totalLength * 100
  return parseFloat(p.toFixed(decimal))
}

export const calcRatio = (totalLength: number, uploadLength: number): number => {
  if (!totalLength || totalLength === 0) return 0
  return parseFloat((uploadLength / totalLength).toFixed(2))
}

export const timeRemaining = (total: number, completed: number, speed: number): number => {
  if (!speed || speed === 0) return Infinity
  return Math.ceil((total - completed) / speed)
}

export const timeFormat = (seconds: number, opts: { prefix?: string; suffix?: string; i18n?: any } = {}): string => {
  const { prefix = '', suffix = '' } = opts
  if (!seconds || seconds <= 0) return '-'
  if (seconds > 86400) return `${prefix}> 1 day${suffix}`
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = seconds % 60
  const parts: string[] = []
  if (h > 0) parts.push(`${h} h`)
  if (m > 0) parts.push(`${m} m`)
  if (s > 0 || parts.length === 0) parts.push(`${s} s`)
  return `${prefix}${parts.join(' ')}${suffix}`
}

export const localeDateTimeFormat = (timestamp: number, locale = 'zh-CN'): string => {
  if (!timestamp) return ''
  return new Date(timestamp).toLocaleDateString(locale)
}

export const ellipsis = (str: string, maxLen = 64): string => {
  if (!str || str.length <= maxLen) return str
  return str.slice(0, maxLen) + '...'
}

export const getFileName = (fullPath: string): string => {
  if (!fullPath) return ''
  return fullPath.replace(/^.*[/\\]/, '')
}

export const getFileExtension = (filename: string): string => {
  if (!filename) return ''
  const m = filename.match(/\.[^./\\]+$/)
  return m ? m[0].toLowerCase() : ''
}

export const removeExtensionDot = (ext: string): string =>
  ext.startsWith('.') ? ext.slice(1) : ext

export const getFileNameFromFile = (file: any): string =>
  getFileName(file?.path || '')

export const getTaskName = (task: any, opts: any = {}): string => {
  const bt = task?.bittorrent
  if (bt?.info?.name) return bt.info.name
  const files = task?.files
  if (files?.length === 1) return getFileNameFromFile(files[0])
  return ''
}

export const checkTaskIsBT = (task: any): boolean => !!task?.bittorrent
export const isMagnetTask = (task: any): boolean => !!(task?.bittorrent && !task.bittorrent.info)
export const checkTaskIsSeeder = (task: any): boolean =>
  !!(task?.bittorrent && task.seeder === 'true')
export const checkTaskTitleIsEmpty = (task: any): boolean => !getTaskName(task)

export const isTorrent = (file: any): boolean => {
  const name = (file?.name || file?.path || '').toLowerCase()
  return name.endsWith('.torrent')
}

export const getAsBase64 = (file: File, callback: (base64: string) => void): void => {
  const reader = new FileReader()
  reader.onload = () => {
    const result = reader.result as string
    const base64 = result.split(',')[1] || result
    callback(base64)
  }
  reader.readAsDataURL(file)
}

export const getTaskUri = (task: any, withTracker = false): string => {
  if (!checkTaskIsBT(task)) {
    return task?.files?.[0]?.uris?.[0]?.uri || ''
  }
  return buildMagnetLink(task, withTracker)
}

export const buildMagnetLink = (task: any, withTracker = false, btTracker = ''): string => {
  const infoHash = task?.infoHash || task?.bittorrent?.info?.infoHash
  if (!infoHash) return ''
  const name = task?.bittorrent?.info?.name || ''
  let magnet = `magnet:?xt=urn:btih:${infoHash}`
  if (name) magnet += `&dn=${encodeURIComponent(name)}`
  if (withTracker && btTracker) {
    btTracker.split(',').forEach((t: string) => { if (t) magnet += `&tr=${encodeURIComponent(t)}` })
  }
  return magnet
}

export const getFileSelection = (files: any[]): string => {
  if (!files?.length) return NONE_SELECTED_FILES
  const selected = files.filter((f) => f.selected === 'true' || f.selected === true)
  if (selected.length === files.length) return SELECTED_ALL_FILES
  if (selected.length === 0) return NONE_SELECTED_FILES
  return selected.map((f) => f.index).join(',')
}

export const listTorrentFiles = (files: any[]): any[] =>
  (files || []).map((f, i) => ({
    ...f,
    idx: i + 1,
    extension: getFileExtension(f.path || '')
  }))

export const mergeTaskResult = (response: any[]): any[] => {
  if (!Array.isArray(response)) return []
  return response.reduce((acc: any[], item) => {
    if (Array.isArray(item)) {
      item.forEach((sub) => {
        if (Array.isArray(sub)) acc.push(...sub)
        else acc.push(sub)
      })
    } else {
      acc.push(item)
    }
    return acc
  }, [])
}

export const decodeThunderLink = (url: string): string => {
  if (!url.toLowerCase().startsWith('thunder://')) return url
  const b64 = url.slice('thunder://'.length)
  const decoded = Buffer.from(b64, 'base64').toString('utf-8')
  return decoded.replace(/^AA/, '').replace(/ZZ$/, '')
}

export const splitTaskLinks = (links: string): string[] => {
  if (!links) return []
  return links
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean)
    .map((l) => (l.toLowerCase().startsWith('thunder://') ? decodeThunderLink(l) : l))
}

export const splitTextRows = (text: string): string[] =>
  text.split('\n').map((l) => l.trim()).filter(Boolean)

export const convertCommaToLine = (str: string): string =>
  str.replace(/,/g, '\n')

export const convertLineToComma = (str: string): string =>
  str.replace(/\n/g, ',')

export const detectResource = (content: string): boolean =>
  RESOURCE_TAGS.some((tag) => content.toLowerCase().includes(tag))

export const buildFileList = (rawFile: File): any[] => [{
  uid: Date.now().toString(),
  name: rawFile.name,
  status: 'ready',
  file: rawFile
}]

export const needCheckCopyright = (links: string): boolean =>
  /\/(av|BV)/i.test(links)

export const isAudioOrVideo = (uri: string): boolean => {
  const ext = getFileExtension(uri)
  return AUDIO_SUFFIXES.includes(ext) || VIDEO_SUFFIXES.includes(ext)
}

export const filterVideoFiles = (files: any[]): any[] =>
  files.filter((f) => VIDEO_SUFFIXES.includes(getFileExtension(f.path || f.name || '')))

export const filterAudioFiles = (files: any[]): any[] =>
  files.filter((f) => AUDIO_SUFFIXES.includes(getFileExtension(f.path || f.name || '')))

export const filterImageFiles = (files: any[]): any[] =>
  files.filter((f) => IMAGE_SUFFIXES.includes(getFileExtension(f.path || f.name || '')))

export const filterDocumentFiles = (files: any[]): any[] =>
  files.filter((f) => DOCUMENT_SUFFIXES.includes(getFileExtension(f.path || f.name || '')))

export const formatOptionsForEngine = (options: Record<string, any> = {}): Record<string, string> => {
  const result: Record<string, string> = {}
  Object.keys(options).forEach((key) => {
    const kebab = kebabCase(key)
    if (Array.isArray(options[key])) {
      result[kebab] = options[key].join('\n')
    } else {
      result[kebab] = `${options[key]}`
    }
  })
  return result
}

export const separateConfig = (options: Record<string, any> = {}): {
  user: Record<string, any>; system: Record<string, any>; others: Record<string, any>
} => {
  const user: Record<string, any> = {}
  const system: Record<string, any> = {}
  const others: Record<string, any> = {}
  Object.keys(options).forEach((key) => {
    if (userKeys.includes(key)) user[key] = options[key]
    else if (systemKeys.includes(key)) system[key] = options[key]
    else others[key] = options[key]
  })
  return { user, system, others }
}

export const changeKeysCase = (obj: Record<string, any>, caseConverter: (s: string) => string): Record<string, any> => {
  if (!isPlainObject(obj)) return obj
  const result: Record<string, any> = {}
  Object.keys(obj).forEach((key) => { result[caseConverter(key)] = obj[key] })
  return result
}

export const changeKeysToCamelCase = (obj: Record<string, any>): Record<string, any> =>
  changeKeysCase(obj, camelCase)

export const changeKeysToKebabCase = (obj: Record<string, any>): Record<string, any> =>
  changeKeysCase(obj, kebabCase)

export const compactUndefined = <T>(arr: (T | undefined)[]): T[] =>
  arr.filter((item): item is T => item !== undefined)

export const checkIsNeedRestart = (changed: Record<string, any>): boolean =>
  Object.keys(changed).some((key) => needRestartKeys.includes(key))

export const checkIsNeedRun = (enable: boolean, lastTime: number, interval: number): boolean => {
  if (!enable) return false
  if (!lastTime) return true
  return Date.now() - lastTime > interval
}

export const parseHeader = (header: string): Record<string, string> => {
  if (!header) return {}
  const result: Record<string, string> = {}
  header.split('\n').forEach((line) => {
    const [key, ...rest] = line.split(':')
    if (key && rest.length) result[camelCase(key.trim())] = rest.join(':').trim()
  })
  return result
}

export const buildRpcUrl = ({ port, secret }: { port: number; secret?: string }): string => {
  const auth = secret ? `token:${secret}@` : ''
  return `http://${auth}127.0.0.1:${port}/jsonrpc`
}

export const generateRandomInt = (min: number, max: number): number =>
  Math.floor(Math.random() * (max - min + 1)) + min

export const intersection = <T>(a1: T[], a2: T[]): T[] =>
  a1.filter((v) => a2.includes(v))

export const cloneArray = <T>(arr: T[], reversed = false): T[] => {
  const copy = [...arr]
  return reversed ? copy.reverse() : copy
}

export const pushItemToFixedLengthArray = <T>(arr: T[], item: T, maxLen: number): T[] => {
  const filtered = arr.filter((i) => i !== item)
  filtered.unshift(item)
  return filtered.slice(0, maxLen)
}

export const removeArrayItem = <T>(arr: T[], item: T): T[] =>
  arr.filter((i) => i !== item)

export const diffConfig = (current: Record<string, any>, next: Record<string, any>): Record<string, any> => {
  const diff: Record<string, any> = {}
  Object.keys(next).forEach((key) => {
    if (JSON.stringify(current[key]) !== JSON.stringify(next[key])) {
      diff[key] = next[key]
    }
  })
  return diff
}

export const getInverseTheme = (theme: string): string =>
  theme === APP_THEME.LIGHT ? APP_THEME.DARK : APP_THEME.LIGHT

export const calcFormLabelWidth = (locale: string): string =>
  locale === 'de' ? '28%' : '25%'

export const isRTL = (locale: string): boolean =>
  SUPPORT_RTL_LOCALES.includes(locale)

export const getLangDirection = (locale: string): 'rtl' | 'ltr' =>
  isRTL(locale) ? 'rtl' : 'ltr'

export const bitfieldToPercent = (text: string): number => {
  if (!text) return 0
  let filled = 0
  for (const ch of text) {
    const n = parseInt(ch, 16)
    if (isNaN(n)) continue
    for (let bit = 3; bit >= 0; bit--) if ((n >> bit) & 1) filled++
  }
  return Math.round((filled / (text.length * 4)) * 100)
}

export const bitfieldToGraphic = (text: string): string => {
  if (!text) return ''
  return text.split('').map((ch) => {
    const n = parseInt(ch, 16)
    if (isNaN(n)) return GRAPHIC[0]
    return GRAPHIC[Math.min(3, Math.round((n / 15) * 3))]
  }).join('')
}

export const changedConfig: { basic: Record<string, any>; advanced: Record<string, any> } = {
  basic: {}, advanced: {}
}
export const backupConfig: { theme: string | undefined; locale: string | undefined } = {
  theme: undefined, locale: undefined
}
