export interface ParsedTorrentFile {
  index: number
  path: string
  length: number
}

export interface ParsedTorrentMeta {
  name: string
  files: ParsedTorrentFile[]
}

type BValue = number | Uint8Array | BValue[] | { [key: string]: BValue }

const decodeText = (bytes: Uint8Array): string => {
  try {
    return new TextDecoder('utf-8').decode(bytes)
  } catch {
    return Array.from(bytes).map((b) => String.fromCharCode(b)).join('')
  }
}

const asDict = (value: BValue | undefined): { [key: string]: BValue } | undefined =>
  value && !Array.isArray(value) && !(value instanceof Uint8Array) && typeof value === 'object'
    ? value as { [key: string]: BValue }
    : undefined

const asList = (value: BValue | undefined): BValue[] =>
  Array.isArray(value) ? value : []

const asNumber = (value: BValue | undefined): number =>
  typeof value === 'number' && Number.isFinite(value) ? value : 0

const asString = (value: BValue | undefined): string =>
  value instanceof Uint8Array ? decodeText(value) : ''

class BencodeReader {
  private offset = 0

  constructor (private readonly input: Uint8Array) {}

  parse (): BValue {
    const token = this.input[this.offset]
    if (token === 100) return this.parseDict()
    if (token === 108) return this.parseList()
    if (token === 105) return this.parseInteger()
    if (token >= 48 && token <= 57) return this.parseBytes()
    throw new Error('Invalid torrent metadata')
  }

  private parseInteger (): number {
    this.offset++
    const end = this.findByte(101)
    const raw = decodeText(this.input.slice(this.offset, end))
    this.offset = end + 1
    return Number(raw)
  }

  private parseList (): BValue[] {
    this.offset++
    const result: BValue[] = []
    while (this.input[this.offset] !== 101) result.push(this.parse())
    this.offset++
    return result
  }

  private parseDict (): { [key: string]: BValue } {
    this.offset++
    const result: { [key: string]: BValue } = {}
    while (this.input[this.offset] !== 101) {
      const key = asString(this.parseBytes())
      result[key] = this.parse()
    }
    this.offset++
    return result
  }

  private parseBytes (): Uint8Array {
    const colon = this.findByte(58)
    const len = Number(decodeText(this.input.slice(this.offset, colon)))
    this.offset = colon + 1
    const end = this.offset + len
    const bytes = this.input.slice(this.offset, end)
    this.offset = end
    return bytes
  }

  private findByte (needle: number): number {
    const end = this.input.indexOf(needle, this.offset)
    if (end < 0) throw new Error('Invalid torrent metadata')
    return end
  }
}

export const parseTorrentMeta = (input: Uint8Array): ParsedTorrentMeta => {
  const root = asDict(new BencodeReader(input).parse())
  const info = asDict(root?.info)
  if (!info) throw new Error('Invalid torrent metadata')

  const name = asString(info.name) || asString(info['name.utf-8']) || 'BT 种子任务'
  const files = asList(info.files)
  if (!files.length) {
    return {
      name,
      files: [{ index: 1, path: name, length: asNumber(info.length) }]
    }
  }

  return {
    name,
    files: files.map((item, idx) => {
      const file = asDict(item)
      const parts = asList(file?.['path.utf-8']).length
        ? asList(file?.['path.utf-8'])
        : asList(file?.path)
      return {
        index: idx + 1,
        path: parts.map((part) => asString(part)).filter(Boolean).join('/') || `${idx + 1}`,
        length: asNumber(file?.length)
      }
    })
  }
}
