import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('book reader provider context', () => {
  it('resolves the stored book account before opening the reader window', () => {
    const librarySource = readFileSync(resolve(process.cwd(), 'src/layout/PageBookLibrary.vue'), 'utf8')
    const readerSource = readFileSync(resolve(process.cwd(), 'src/layout/BookReaderModal.vue'), 'utf8')

    expect(librarySource).toContain('const token = book.user_id && book.user_id !== \'local\' ? await UserDAL.GetUserTokenFromDB(book.user_id) : undefined')
    expect(librarySource).toContain('const readerBook = tokenfrom === \'unknown\' ? book : { ...book, tokenfrom }')
    expect(readerSource).toContain("'other', 'Origin', '', book.tokenfrom")
  })
})
