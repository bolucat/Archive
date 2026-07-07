const READER_FORMATS = new Set([
  'epub',
  'mobi',
  'azw',
  'azw3',
  'fb2',
  'cbz',
  'cbr',
  'cbt',
  'cb7',
  'docx',
  'html',
  'htm',
  'pdf',
  'txt',
  'md',
  'markdown'
])

const LEGACY_SCAN_ONLY_FORMATS = new Set([
  'azw4',
  'djvu',
  'caj',
  'chm',
  'umd'
])

export function normalizeBookExt(ext = ''): string {
  return ext.trim().replace(/^\./, '').toLowerCase()
}

export function isReaderFormat(ext = ''): boolean {
  return READER_FORMATS.has(normalizeBookExt(ext))
}

export function isReadableBookFormat(ext = ''): boolean {
  return isReaderFormat(ext)
}

export function isLegacyScanOnlyBookFormat(ext = ''): boolean {
  return LEGACY_SCAN_ONLY_FORMATS.has(normalizeBookExt(ext))
}

export function isScannableBookFormat(ext = ''): boolean {
  return isReadableBookFormat(ext) || isLegacyScanOnlyBookFormat(ext)
}

export function getFormat(ext = ''): string {
  const normalized = normalizeBookExt(ext)
  if (normalized === 'htm') return 'HTML'
  if (normalized === 'markdown') return 'MD'
  return normalized.toUpperCase()
}
