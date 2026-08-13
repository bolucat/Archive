import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('cloud book open contracts', () => {
  it('opens every reader-supported cloud book format through the provider-neutral reader', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/utils/openfile.ts'), 'utf8')

    expect(source).toContain("if (getBookFileExt(file) !== 'pdf' && isReaderFormat(getBookFileExt(file))) {")
    expect(source).toContain("window.WebOpenWindow({ page: 'PageBookReader', data: book, theme: 'dark' })")
    const readerSource = readFileSync(resolve(process.cwd(), 'src/layout/BookReaderModal.vue'), 'utf8')
    expect(readerSource).toContain("getRawUrl(book.user_id, book.drive_id, book.file_id")
  })

  it('keeps Baidu download headers with the returned book URL', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/cloudbaidu/adapter.ts'), 'utf8')

    expect(source).toContain("'User-Agent': 'pan.baidu.com'")
    expect(source).toContain("Referer: 'https://pan.baidu.com/'")
  })

  it('allows text previews to consume third-party signed URLs with their headers', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/aliapi/file.ts'), 'utf8')

    expect(source).toContain('AliHttp.GetString(downUrl.url, user_id, filesize, maxsize, downUrl.headers || {}, true)')
  })

  it('turns a stalled cloud download into a recoverable reader error', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/utils/bookReader.ts'), 'utf8')

    expect(source).toContain('const BOOK_FETCH_TIMEOUT_MS = 60000')
    expect(source).toContain('signal: controller.signal')
    expect(source).toContain("throw new Error('书籍下载超时，请重试')")
  })
})
