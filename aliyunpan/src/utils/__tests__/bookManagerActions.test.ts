import { describe, expect, it } from 'vitest'
import {
  buildKoodoNoteDeepLink,
  getManagerHeaderActions,
  isKoodoImportableFile,
  shouldShowProtectionOverlay,
  summarizeDroppedFiles,
  validateOpdsUrl,
} from '../bookManagerActions'

describe('bookManagerActions', () => {
  it('lists non-sync Koodo manager header actions in stable order', () => {
    expect(getManagerHeaderActions().map((item) => item.key)).toEqual([
      'import-local',
      'scan-cloud',
      'opds',
      'sort',
      'view-mode',
      'settings',
    ])
  })

  it('excludes account and sync actions from manager header actions', () => {
    expect(getManagerHeaderActions().map((item) => item.key)).not.toContain('login')
    expect(getManagerHeaderActions().map((item) => item.key)).not.toContain('sync')
  })

  it('matches Koodo note deep link format', () => {
    expect(buildKoodoNoteDeepLink('note 1')).toBe('koodo-reader://open-note?noteKey=note%201')
  })

  it('accepts Koodo-supported local file formats', () => {
    expect(isKoodoImportableFile('Novel.epub')).toBe(true)
    expect(isKoodoImportableFile('Manual.PDF')).toBe(true)
    expect(isKoodoImportableFile('archive.zip')).toBe(false)
  })

  it('summarizes dropped files with supported and rejected lists', () => {
    const result = summarizeDroppedFiles(['a.epub', 'b.txt', 'c.mov'])
    expect(result.supported).toEqual(['a.epub', 'b.txt'])
    expect(result.rejected).toEqual(['c.mov'])
  })
})

describe('manager import and protection helpers', () => {
  it('validates OPDS urls', () => {
    expect(validateOpdsUrl('https://example.com/opds')).toEqual({ ok: true, url: 'https://example.com/opds' })
    expect(validateOpdsUrl('ftp://example.com')).toEqual({ ok: false, error: 'Only http and https OPDS urls are supported' })
    expect(validateOpdsUrl('not a url')).toEqual({ ok: false, error: 'Invalid OPDS url' })
  })

  it('shows protection overlay only when enabled and locked', () => {
    expect(shouldShowProtectionOverlay({ enabled: true, unlocked: false })).toBe(true)
    expect(shouldShowProtectionOverlay({ enabled: true, unlocked: true })).toBe(false)
    expect(shouldShowProtectionOverlay({ enabled: false, unlocked: false })).toBe(false)
  })
})
