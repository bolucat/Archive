import { describe, expect, it, vi } from 'vitest'
import { buildExternalBookMetadataOutcomePatch, buildExternalBookMetadataPatch, canHydrateExternalBookMetadata, lookupExternalBookMetadata, lookupExternalBookMetadataResult } from '../bookExternalMetadata'
import type { IBookItem } from '../../types/book'

function book(overrides: Partial<IBookItem> = {}): IBookItem {
  return { id: 'book', user_id: 'user', drive_id: 'drive', file_id: 'file', parent_file_id: 'root', file_name: '三体.epub', ext: 'epub', size: 1024, category: 'book', title: '三体', author: '刘慈欣', scanned_at: 1, ...overrides }
}

describe('book external metadata', () => {
  it('uses the configured Google Books key for title and author search', async () => {
    const requestMock = vi.fn(async (_url: string) => ({ ok: true, json: async () => ({ items: [{ volumeInfo: { title: '三体', authors: ['刘慈欣'], imageLinks: { thumbnail: 'https://books.google.test/cover.jpg' }, industryIdentifiers: [{ identifier: '9787536692930' }] } }] }) }))
    await expect(lookupExternalBookMetadata(book(), requestMock as unknown as typeof fetch)).resolves.toMatchObject({ title: '三体', source: 'googlebooks', coverUrl: 'https://books.google.test/cover.jpg' })
    const url = String(requestMock.mock.calls[0][0])
    expect(url).toContain('www.googleapis.com/books/v1/volumes')
    expect(url).toContain('intitle%3A%E4%B8%89%E4%BD%93')
    expect(url).toContain('inauthor%3A%E5%88%98%E6%85%88%E6%AC%A3')
    expect(new URL(url).searchParams.get('key')).toBeTruthy()
  })

  it('makes one Google Books request per lookup', async () => {
    const requestMock = vi.fn(async () => ({ ok: true, json: async () => ({ items: [{ volumeInfo: { title: 'The Wailing Wind', authors: ['Tony Hillerman'], imageLinks: { thumbnail: 'https://books.google.test/cover.jpg' } } }] }) }))
    const logs: string[] = []
    await expect(lookupExternalBookMetadata(book({ file_name: 'The Wailing Wind - Tony Hillerman.mobi', title: 'The Wailing Wind', author: 'Tony Hillerman' }), requestMock as unknown as typeof fetch, (message) => logs.push(message))).resolves.toMatchObject({ source: 'googlebooks' })
    expect(requestMock).toHaveBeenCalledTimes(1)
    expect(logs.some((message) => message.includes('Google Books 命中'))).toBe(true)
  })

  it('rejects low confidence results and still enriches books that already have a thumbnail', async () => {
    const request = vi.fn(async () => ({ ok: true, json: async () => ({ items: [{ volumeInfo: { title: '不同的书', authors: ['其他作者'] } }] }) })) as unknown as typeof fetch
    await expect(lookupExternalBookMetadataResult(book(), request)).resolves.toEqual({ status: 'no-match' })
    expect(canHydrateExternalBookMetadata(book({ cover_url: 'https://cover.example/a.jpg' }))).toBe(true)
    expect(buildExternalBookMetadataPatch({ title: 'Title', coverUrl: 'https://cover.example/a.jpg' }, 123)).toMatchObject({ metadata_source: 'googlebooks', metadata_updated_at: 123, cover_url: 'https://cover.example/a.jpg' })
  })

  it('reports lookup outcomes to the caller supplied diagnostics logger', async () => {
    const source = await import('node:fs').then(({ readFileSync }) => readFileSync(new URL('../bookExternalMetadata.ts', import.meta.url), 'utf8'))

    expect(source).toContain('ExternalBookMetadataLogger')
    expect(source).toContain('www.googleapis.com/books/v1/volumes')
    expect(source).toContain('GOOGLE_BOOKS_API_TOKEN')
    expect(source).not.toContain('archive.org/advancedsearch.php')
    expect(source).toContain('const EXTERNAL_BOOK_METADATA_TIMEOUT_MS = 6000')
    expect(source).toContain('log?.(`${logPrefix} Google Books 请求失败`, error)')
    expect(source).toContain('log?.(`${logPrefix} Google Books 命中：')
  })

  it('rejects an exact title when a known author explicitly disagrees', async () => {
    const request = vi.fn(async () => ({ ok: true, json: async () => ({ items: [{ volumeInfo: { title: '三体', authors: ['完全不同的作者'], imageLinks: { thumbnail: 'https://books.google.test/wrong.jpg' } } }] }) })) as unknown as typeof fetch

    await expect(lookupExternalBookMetadataResult(book(), request)).resolves.toEqual({ status: 'no-match' })
  })

  it('persists a terminal no-match marker so the next app launch does not spend quota again', () => {
    const patch = buildExternalBookMetadataOutcomePatch({ status: 'no-match' }, 456)

    expect(patch).toEqual({ metadata_source: 'googlebooks-no-match', metadata_updated_at: 456 })
    expect(canHydrateExternalBookMetadata(book(patch || {}))).toBe(false)
  })

  it('pauses additional lookups after a transient Google Books failure', async () => {
    const unavailable = vi.fn(async () => { throw new Error('HTTP 503') })
    const skipped = vi.fn()
    await expect(lookupExternalBookMetadataResult(book(), unavailable as unknown as typeof fetch)).resolves.toMatchObject({ status: 'retry-later', retryAt: expect.any(Number) })
    await expect(lookupExternalBookMetadata(book({ id: 'second' }), skipped as unknown as typeof fetch)).resolves.toBeNull()
    expect(unavailable).toHaveBeenCalledTimes(1)
    expect(skipped).not.toHaveBeenCalled()
  })
})
