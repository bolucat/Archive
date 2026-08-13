import type { IBookItem } from '../types/book'

export interface IBookWork {
  book: IBookItem
  variants: IBookItem[]
}

const FORMAT_PRIORITY = ['EPUB', 'AZW3', 'AZW', 'MOBI', 'PDF', 'FB2', 'TXT']

function normalized(value: string): string {
  return value
    .normalize('NFKC')
    .toLocaleLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, '')
}

export function bookWorkKey(book: IBookItem): string {
  const title = normalized(book.title || book.file_name.replace(/\.[^.]+$/, ''))
  const author = normalized(book.author || '')
  return `${title}|${author === normalized('未知作者') ? '' : author}`
}

function compareVariants(a: IBookItem, b: IBookItem): number {
  const aPriority = FORMAT_PRIORITY.indexOf((a.ext || '').toUpperCase())
  const bPriority = FORMAT_PRIORITY.indexOf((b.ext || '').toUpperCase())
  const aRank = aPriority < 0 ? FORMAT_PRIORITY.length : aPriority
  const bRank = bPriority < 0 ? FORMAT_PRIORITY.length : bPriority
  return aRank - bRank || (b.scanned_at || 0) - (a.scanned_at || 0)
}

export function groupBookWorks(books: IBookItem[]): IBookWork[] {
  const groups = new Map<string, IBookItem[]>()
  for (const book of books) {
    const key = bookWorkKey(book)
    const variants = groups.get(key)
    if (variants) variants.push(book)
    else groups.set(key, [book])
  }
  return Array.from(groups.values()).map((variants) => {
    const sorted = [...variants].sort(compareVariants)
    const primary = sorted[0]
    const cover = sorted.find((book) => book.cover_url || book.thumbnail)
    return {
      book: cover && cover.id !== primary.id ? { ...primary, cover_url: cover.cover_url, thumbnail: cover.thumbnail } : primary,
      variants: sorted
    }
  })
}
