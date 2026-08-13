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

  it('persists the reading position before Exit closes the reader', () => {
    const modal = read('src/layout/BookReaderModal.vue')
    const closeStart = modal.indexOf('async function close()')
    const saveStart = modal.indexOf('await saveBookPosition(true)', closeStart)
    const emitStart = modal.indexOf("emit('update:visible', false)", closeStart)

    expect(closeStart).toBeGreaterThan(-1)
    expect(saveStart).toBeGreaterThan(closeStart)
    expect(emitStart).toBeGreaterThan(saveStart)
  })

  it('opens books with their latest persisted reading position', () => {
    const library = read('src/layout/PageBookLibrary.vue')
    expect(library).toContain("import DB from '../utils/db'")
    expect(library).toContain('const savedBook = (await DB.getBookItemsByIds([book.id]).catch(() => []))[0]')
  })

  it('labels pagination as chapter-relative', () => {
    const widget = read('src/layout/book-reader/ReaderPageWidget.vue')
    expect(widget).toContain('Chapter Page ${currentPage}')
    expect(widget).toContain('Chapter Page ${currentPage * 2 - 1}')
  })
})
