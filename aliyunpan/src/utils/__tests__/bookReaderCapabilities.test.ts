import { describe, expect, it } from 'vitest'
import {
  isReaderFormat,
  isComicBookFormat,
  isFixedLayoutBookFormat,
  isLegacyScanOnlyBookFormat,
  isReadableBookFormat,
  isScannableBookFormat
} from '../bookReaderCapabilities'

describe('bookReaderCapabilities', () => {
  it('all formerly-native formats now route to Reader', () => {
    for (const ext of ['pdf', 'txt', 'md', 'markdown']) {
      expect(isReaderFormat(ext)).toBe(true)
      expect(isReadableBookFormat(ext)).toBe(true)
    }
  })

  it('routes Reader formats to Reader', () => {
    for (const ext of ['epub', 'mobi', 'azw', 'azw3', 'fb2', 'cbz', 'cbr', 'cbt', 'cb7', 'docx', 'html']) {
      expect(isReaderFormat(ext)).toBe(true)
      expect(isReadableBookFormat(ext)).toBe(true)
    }
  })

  it('rejects unsupported formats', () => {
    expect(isReadableBookFormat('zip')).toBe(false)
    expect(isScannableBookFormat('zip')).toBe(false)
    expect(isReaderFormat('zip')).toBe(false)
  })

  it('treats legacy formats as scan-only but not readable', () => {
    for (const ext of ['azw4', 'djvu', 'caj', 'chm', 'umd']) {
      expect(isLegacyScanOnlyBookFormat(ext)).toBe(true)
      expect(isReadableBookFormat(ext)).toBe(false)
      expect(isScannableBookFormat(ext)).toBe(true)
    }
  })

  it('scans every readable format', () => {
    for (const ext of ['epub', 'pdf', 'txt', 'md', 'markdown', 'mobi', 'azw', 'azw3', 'fb2', 'cbz', 'cbr', 'cbt', 'cb7', 'docx', 'html', 'htm']) {
      expect(isScannableBookFormat(ext)).toBe(true)
    }
  })

  it('distinguishes fixed-layout page totals from reflowable chapter pages', () => {
    for (const ext of ['pdf', 'cbz', 'cbr', 'cbt', 'cb7']) expect(isFixedLayoutBookFormat(ext)).toBe(true)
    for (const ext of ['epub', 'mobi', 'azw3', 'txt', 'md', 'docx']) expect(isFixedLayoutBookFormat(ext)).toBe(false)
  })

  it('identifies archive-based comic books separately from PDFs', () => {
    for (const ext of ['cbz', 'cbr', 'cbt', 'cb7']) expect(isComicBookFormat(ext)).toBe(true)
    for (const ext of ['pdf', 'epub', 'mobi', 'txt']) expect(isComicBookFormat(ext)).toBe(false)
  })
})
