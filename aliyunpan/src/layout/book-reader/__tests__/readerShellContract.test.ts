import { describe, expect, it } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'

const read = (file: string) => fs.readFileSync(path.join(process.cwd(), file), 'utf8')

describe('ReaderShell contract', () => {
  it('defines all four Koodo edge panels', () => {
    const shell = read('src/layout/book-reader/ReaderShell.vue')
    expect(shell).toContain('reader-edge-top')
    expect(shell).toContain('reader-edge-left')
    expect(shell).toContain('reader-edge-right')
    expect(shell).toContain('reader-edge-bottom')
  })

  it('keeps a shared Koodo-style panel button', () => {
    const button = read('src/layout/book-reader/ReaderPanelButton.vue')
    expect(button).toContain('reader-panel-button')
    expect(button).toContain(':title="title"')
  })

  it('is used by BookReaderModal', () => {
    const modal = read('src/layout/BookReaderModal.vue')
    expect(modal).toContain("import ReaderShell from './book-reader/ReaderShell.vue'")
    expect(modal).toContain('<ReaderShell')
  })
})
