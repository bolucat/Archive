import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const read = (path: string) => readFileSync(resolve(process.cwd(), path), 'utf8')

describe('book library large collection safeguards', () => {
  it('keeps the large book collection shallowly reactive', () => {
    const source = read('src/store/booklibrary.ts')

    expect(source).toContain("import { computed, ref, shallowRef } from 'vue'")
    expect(source).toContain('const books = shallowRef<IBookItem[]>([])')
  })

  it('bounds thumbnail hydration and applies its results in one store update', () => {
    const source = read('src/store/booklibrary.ts')

    expect(source).toContain('const BOOK_THUMBNAIL_HYDRATE_LIMIT = 72')
    expect(source).toContain('const BOOK_THUMBNAIL_HYDRATE_CONCURRENCY = 6')
    expect(source).toContain('.slice(0, BOOK_THUMBNAIL_HYDRATE_LIMIT)')
    expect(source).toContain('for (let index = 0; index < sources.length; index += BOOK_THUMBNAIL_HYDRATE_CONCURRENCY)')
    expect(source).toContain('if (updates.size) {\n      books.value = books.value.map')
    expect(source).not.toContain('books.value = books.value.map((book, currentIndex)')
  })

  it('renders the collection in batches instead of mounting every matching book', () => {
    const source = read('src/layout/PageBookLibrary.vue')

    expect(source).toContain('const bookRenderLimit = ref(240)')
    expect(source).toContain('const renderedBooks = computed(() => readerVisibleBooks.value.slice(0, bookRenderLimit.value))')
    expect(source).toContain("v-for='book in renderedBooks'")
    expect(source).toContain('显示更多（已显示')
  })

  it('recognizes title-author filenames that start with a catalogue number', () => {
    const source = read('src/store/booklibrary.ts')
    const parserSource = read('src/utils/bookFilenameMeta.ts')

    expect(source).toContain("export { parseBookMeta } from '../utils/bookFilenameMeta'")
    expect(parserSource).toContain('const CATALOG_PREFIX_RE')
    expect(parserSource).toContain('CATALOG_PREFIX_RE.test(raw)')
    expect(parserSource).toContain('title: left || raw')
    expect(parserSource).toContain('author: right || \'未知作者\'')
  })
})
