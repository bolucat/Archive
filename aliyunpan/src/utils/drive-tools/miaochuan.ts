export interface MiaochuanFile {
  path: string
  name: string
  size: number
  md5?: string
  gcid?: string
  sourceProvider?: string
  raw?: any
}

export interface MiaochuanNormalizeResult {
  files: MiaochuanFile[]
  errors: string[]
  totalSize: number
  report: string
}

const MD5_RE = /^[a-f0-9]{32}$/i
const GCID_RE = /^[a-f0-9]{40}$/i

const asString = (value: any): string => value == null ? '' : String(value).trim()

const asNumber = (value: any): number => {
  if (typeof value === 'number' && Number.isFinite(value)) return value
  const text = asString(value).replace(/,/g, '')
  const n = Number(text)
  return Number.isFinite(n) ? n : 0
}

const firstValue = (item: any, keys: string[]) => {
  for (const key of keys) {
    const value = item?.[key]
    if (value !== undefined && value !== null && value !== '') return value
  }
  return undefined
}

const decodeBase64Bytes = (value: string): number[] => {
  const text = value.replace(/-/g, '+').replace(/_/g, '/')
  const padded = text + '='.repeat((4 - (text.length % 4)) % 4)
  if (typeof atob === 'function') return Array.from(atob(padded), c => c.charCodeAt(0))
  if (typeof Buffer !== 'undefined') return Array.from(Buffer.from(padded, 'base64'))
  return []
}

export const normalizeMiaochuanMd5 = (value: any): string => {
  const text = asString(value)
  if (!text) return ''
  const lower = text.toLowerCase()
  if (MD5_RE.test(lower)) return lower

  const bytes = decodeBase64Bytes(text)
  if (bytes.length === 16) return bytes.map(b => b.toString(16).padStart(2, '0')).join('')

  // guangya-cloud-helper accepts an older 32-char g-v nibble encoding.
  if (/^[g-v]{32}$/i.test(text)) {
    return text.toLowerCase().replace(/[g-v]/g, ch => (ch.charCodeAt(0) - 103).toString(16))
  }
  return ''
}

const collectRows = (input: any): any[] => {
  if (Array.isArray(input)) return input
  if (!input || typeof input !== 'object') return []
  for (const key of ['files', 'fileList', 'items', 'list', 'resources', 'data']) {
    const value = input[key]
    if (Array.isArray(value)) return value
    if (value && typeof value === 'object') {
      const nested = collectRows(value)
      if (nested.length) return nested
    }
  }
  return [input]
}

const normalizePath = (item: any): { path: string; name: string } => {
  const rawPath = asString(firstValue(item, ['path', '__gypPath', 'filePath', 'fullPath', 'localPath', 'server_path', 'serverPath']))
  const rawName = asString(firstValue(item, ['name', 'fileName', 'filename', 'title']))
  let path = (rawPath || rawName).replace(/\\/g, '/').replace(/^\/+/, '').replace(/\/+/g, '/')
  if (!path && rawName) path = rawName
  const parts = path.split('/').filter(Boolean)
  const name = parts.at(-1) || rawName
  if (!path && name) path = name
  return { path, name }
}

const providerOf = (item: any): string => asString(firstValue(item, ['provider', 'sourceProvider', 'source', 'pan', 'drive'])) || 'unknown'

export const normalizeMiaochuanPayload = (payload: string | any): MiaochuanNormalizeResult => {
  let data = payload
  const errors: string[] = []
  if (typeof payload === 'string') {
    try {
      data = JSON.parse(payload)
    } catch (error: any) {
      return { files: [], errors: [`JSON 解析失败：${error?.message || '格式不正确'}`], totalSize: 0, report: 'JSON 解析失败' }
    }
  }

  const files: MiaochuanFile[] = []
  const rows = collectRows(data)
  rows.forEach((item, index) => {
    const { path, name } = normalizePath(item)
    const size = asNumber(firstValue(item, ['size', 'fileSize', 'file_size', 'bytes', 'length']))
    const gcid = asString(firstValue(item, ['gcid', 'gcId', 'GCID'])).toLowerCase()
    const contentHashName = asString(item?.content_hash_name || item?.contentHashName).toLowerCase()
    const md5Source = firstValue(item, ['etag', 'md5', 'fileMd5', 'file_md5', 'contentMd5', 'content_md5', 'sourceMd5', 'source_md5', 'block_list_md5'])
      || (contentHashName === 'md5' ? item?.content_hash || item?.contentHash : '')
    const md5 = normalizeMiaochuanMd5(md5Source)

    if (!path || !name) {
      errors.push(`第 ${index + 1} 项缺少文件名或路径`)
      return
    }
    if (!size) {
      errors.push(`第 ${index + 1} 项缺少文件大小：${path}`)
      return
    }
    if (!md5 && !GCID_RE.test(gcid)) {
      errors.push(`第 ${index + 1} 项缺少有效 MD5/GCID：${path}`)
      return
    }
    files.push({ path, name, size, md5: md5 || undefined, gcid: GCID_RE.test(gcid) ? gcid : undefined, sourceProvider: providerOf(item), raw: item })
  })

  const totalSize = files.reduce((sum, item) => sum + item.size, 0)
  const report = [
    `识别到 ${files.length} 个可秒传文件`,
    errors.length ? `跳过 ${errors.length} 项：${errors.slice(0, 5).join('；')}${errors.length > 5 ? '…' : ''}` : '没有发现格式错误',
    `来源：${Array.from(new Set(files.map(f => f.sourceProvider || 'unknown'))).join('、')}`
  ].join('\n')
  return { files, errors, totalSize, report }
}
