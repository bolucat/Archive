import type { IBookItem } from '../types/book'
import { runRateLimitedScanRequest } from './libraryScanRateLimiter'
import { GOOGLE_BOOKS_API_TOKEN } from '../secrets.generated'

export interface ExternalBookMetadata {
  title?: string
  author?: string
  summary?: string
  coverUrl?: string
  isbn?: string
  publisher?: string
  publishedDate?: string
  language?: string
  subjects?: string[]
  source?: 'googlebooks'
}

export type ExternalBookMetadataLookupResult =
  | { status: 'matched'; metadata: ExternalBookMetadata }
  | { status: 'no-match' }
  | { status: 'retry-later'; retryAt: number }
  | { status: 'unavailable' }

type GoogleBooksVolume = {
  volumeInfo?: {
    title?: string
    authors?: string[]
    description?: string
    imageLinks?: { thumbnail?: string; smallThumbnail?: string }
    industryIdentifiers?: Array<{ identifier?: string }>
    publisher?: string
    publishedDate?: string
    language?: string
    categories?: string[]
  }
}

type BookMetadataCandidate = {
  title?: string
  authors?: string[]
  isbn?: string[]
}

const GOOGLE_BOOKS_SEARCH_URL = 'https://www.googleapis.com/books/v1/volumes'
const EXTERNAL_BOOK_METADATA_TIMEOUT_MS = 6000
const GOOGLE_BOOKS_FAILURE_COOLDOWN_MS = 5 * 60 * 1000
const UNKNOWN_AUTHOR = new Set(['', '未知作者', 'unknown author'])
export type ExternalBookMetadataLogger = (message: string, error?: unknown) => void

let googleBooksUnavailableUntil = 0

function normalized(value = ''): string {
  return value.toLowerCase().replace(/[\s\p{P}\p{S}_]+/gu, '')
}

function firstText(value: string | string[] | undefined): string {
  return Array.isArray(value) ? String(value[0] || '') : String(value || '')
}

function buildGoogleBooksSearchUrl(book: IBookItem): string {
  const params = new URLSearchParams({ maxResults: '5', projection: 'full', key: GOOGLE_BOOKS_API_TOKEN })
  const isbn = String(book.isbn || '').replace(/[^0-9Xx]/g, '')
  if (/^(?:97[89]\d{10}|\d{9}[\dX])$/i.test(isbn)) params.set('q', `isbn:${isbn}`)
  else {
    const title = book.title || book.file_name.replace(/\.[^.]+$/, '')
    const author = UNKNOWN_AUTHOR.has(normalized(book.author || '')) ? '' : book.author || ''
    params.set('q', [title && `intitle:${title}`, author && `inauthor:${author}`].filter(Boolean).join('+'))
  }
  return `${GOOGLE_BOOKS_SEARCH_URL}?${params.toString()}`
}

function matchScore(book: IBookItem, candidate: BookMetadataCandidate): number {
  const isbn = String(book.isbn || '').replace(/[^0-9Xx]/g, '').toLowerCase()
  if (isbn && candidate.isbn?.some((value) => String(value).replace(/[^0-9Xx]/g, '').toLowerCase() === isbn)) return 100
  const title = normalized(book.title || book.file_name.replace(/\.[^.]+$/, ''))
  const candidateTitle = normalized(candidate.title || '')
  if (!title || !candidateTitle) return 0
  let score = title === candidateTitle ? 75 : (candidateTitle.includes(title) || title.includes(candidateTitle) ? 50 : 0)
  const author = normalized(book.author || '')
  const authors = (candidate.authors || []).map((value) => normalized(value))
  if (!UNKNOWN_AUTHOR.has(author) && authors.length) {
    if (!authors.some((value) => value === author || value.includes(author) || author.includes(value))) return 0
    score += 25
  }
  return score
}

async function lookupGoogleBooksMetadata(book: IBookItem, request: typeof fetch): Promise<ExternalBookMetadata | null> {
  const response = await request(buildGoogleBooksSearchUrl(book), { signal: AbortSignal.timeout(EXTERNAL_BOOK_METADATA_TIMEOUT_MS) })
  if (!response.ok) throw new Error(`HTTP ${response.status}`)
  const body = await response.json() as { items?: GoogleBooksVolume[] }
  const candidate = (body.items || [])
    .map((item) => {
      const info = item.volumeInfo || {}
      return { info, score: matchScore(book, { title: info.title, authors: info.authors, isbn: info.industryIdentifiers?.map((item) => item.identifier || '') }) }
    })
    .sort((a, b) => b.score - a.score)[0]
  if (!candidate || candidate.score < 75) return null
  const info = candidate.info
  const coverUrl = info.imageLinks?.thumbnail || info.imageLinks?.smallThumbnail || ''
  return {
    title: info.title,
    author: firstText(info.authors),
    summary: info.description,
    coverUrl: coverUrl.replace(/^http:/, 'https:'),
    isbn: firstText(info.industryIdentifiers?.map((item) => item.identifier || '')),
    publisher: info.publisher,
    publishedDate: info.publishedDate,
    language: info.language,
    subjects: info.categories?.slice(0, 8),
    source: 'googlebooks'
  }
}

function isGoogleBooksTemporaryFailure(error: unknown): boolean {
  const message = String(error instanceof Error ? error.message : error || '')
  return /HTTP (?:429|5\d\d)|signal timed out|TimeoutError/i.test(message)
}

export function canHydrateExternalBookMetadata(book: IBookItem): boolean {
  return !String(book.metadata_source || '').startsWith('googlebooks') && !!(book.title || book.file_name)
}

export async function lookupExternalBookMetadataResult(book: IBookItem, request: typeof fetch = fetch, log?: ExternalBookMetadataLogger): Promise<ExternalBookMetadataLookupResult> {
  const logPrefix = `[book-metadata] ${book.ext.toUpperCase()} ${book.file_name}`
  if (!GOOGLE_BOOKS_API_TOKEN) {
    log?.(`${logPrefix} 未配置 GOOGLE_BOOKS_API_TOKEN，跳过 Google Books 查询`)
    return { status: 'unavailable' }
  }
  if (googleBooksUnavailableUntil > Date.now()) {
    log?.(`${logPrefix} Google Books 暂不可用，稍后自动重试`)
    return { status: 'retry-later', retryAt: googleBooksUnavailableUntil }
  }
  try {
    log?.(`${logPrefix} 查询 Google Books：title=${book.title || '-'} author=${book.author || '-'} isbn=${book.isbn || '-'}`)
    const meta = await runRateLimitedScanRequest('external:googlebooks', () => lookupGoogleBooksMetadata(book, request))
    if (!meta) {
      log?.(`${logPrefix} Google Books 未命中`)
      return { status: 'no-match' }
    }
    log?.(`${logPrefix} Google Books 命中：${meta.title || '-'}，封面=${meta.coverUrl ? '有' : '无'}`)
    return { status: 'matched', metadata: meta }
  } catch (error) {
    if (isGoogleBooksTemporaryFailure(error)) {
      googleBooksUnavailableUntil = Date.now() + GOOGLE_BOOKS_FAILURE_COOLDOWN_MS
      log?.(`${logPrefix} Google Books 请求失败`, error)
      return { status: 'retry-later', retryAt: googleBooksUnavailableUntil }
    }
    log?.(`${logPrefix} Google Books 请求失败`, error)
    return { status: 'unavailable' }
  }
}

export async function lookupExternalBookMetadata(book: IBookItem, request: typeof fetch = fetch, log?: ExternalBookMetadataLogger): Promise<ExternalBookMetadata | null> {
  const result = await lookupExternalBookMetadataResult(book, request, log)
  return result.status === 'matched' ? result.metadata : null
}

export function buildExternalBookMetadataPatch(meta: ExternalBookMetadata, now = Date.now()): Partial<IBookItem> {
  const patch: Partial<IBookItem> = { metadata_source: meta.source || 'googlebooks', metadata_updated_at: now }
  if (meta.title) patch.title = meta.title
  if (meta.author) patch.author = meta.author
  if (meta.summary) patch.summary = meta.summary
  if (meta.coverUrl) patch.cover_url = meta.coverUrl
  if (meta.isbn) patch.isbn = meta.isbn
  if (meta.publisher) patch.publisher = meta.publisher
  if (meta.publishedDate) patch.published_date = meta.publishedDate
  if (meta.language) patch.language = meta.language
  if (meta.subjects?.length) patch.subjects = meta.subjects
  return patch
}

export function buildExternalBookMetadataOutcomePatch(result: ExternalBookMetadataLookupResult, now = Date.now()): Partial<IBookItem> | null {
  if (result.status === 'matched') return buildExternalBookMetadataPatch(result.metadata, now)
  if (result.status === 'no-match') return { metadata_source: 'googlebooks-no-match', metadata_updated_at: now }
  return null
}
