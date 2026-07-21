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
})
