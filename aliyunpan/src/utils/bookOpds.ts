import { XMLParser } from 'fast-xml-parser'

export interface OpdsBookEntry {
  title: string
  author: string
  href: string
  mime: string
}

type XmlNode = Record<string, unknown>

const parser = new XMLParser({
  attributeNamePrefix: '@_',
  ignoreAttributes: false,
  trimValues: true,
})

function asArray<T>(value: T | T[] | undefined): T[] {
  if (!value) return []
  return Array.isArray(value) ? value : [value]
}

function isNode(value: unknown): value is XmlNode {
  return !!value && typeof value === 'object' && !Array.isArray(value)
}

function child(node: XmlNode, name: string): unknown {
  const target = name.toLowerCase()
  const entry = Object.entries(node).find(([key]) => key.toLowerCase().split(':').pop() === target)
  return entry?.[1]
}

function readText(value: unknown): string {
  if (typeof value === 'string' || typeof value === 'number') return String(value).trim()
  if (isNode(value)) return readText(value['#text'])
  return ''
}

function attr(node: XmlNode, name: string): string {
  const value = node[`@_${name}`]
  return typeof value === 'string' || typeof value === 'number' ? String(value).trim() : ''
}

function fileNameFromHref(href: string): string {
  try {
    const url = new URL(href)
    const segment = url.pathname.split('/').filter(Boolean).pop() || href
    return decodeURIComponent(segment).replace(/\.[^.]+$/, '') || href
  } catch {
    const segment = href.split('/').filter(Boolean).pop() || href
    return decodeURIComponent(segment).replace(/\.[^.]+$/, '') || href
  }
}

export function parseOpdsFeed(xml: string): OpdsBookEntry[] {
  const parsed = parser.parse(xml)
  if (!isNode(parsed)) return []

  const feed = child(parsed, 'feed')
  if (!isNode(feed)) return []

  return asArray(child(feed, 'entry'))
    .filter(isNode)
    .map((entry) => {
      const acquisition = asArray(child(entry, 'link'))
        .filter(isNode)
        .find((link) => attr(link, 'href') && attr(link, 'rel').includes('acquisition'))
      if (!acquisition) return null

      const href = attr(acquisition, 'href')
      const author = child(entry, 'author')
      const authorName = isNode(author) ? readText(child(author, 'name')) : readText(author)

      return {
        title: readText(child(entry, 'title')) || fileNameFromHref(href),
        author: authorName || 'Unknown Author',
        href,
        mime: attr(acquisition, 'type') || 'application/octet-stream',
      }
    })
    .filter((entry): entry is OpdsBookEntry => !!entry)
}

export async function fetchOpdsFeed(url: string): Promise<OpdsBookEntry[]> {
  const response = await fetch(url, {
    headers: {
      Accept: 'application/atom+xml, application/xml, text/xml;q=0.9, */*;q=0.8',
    },
  })
  if (!response.ok) throw new Error(`OPDS request failed: ${response.status}`)
  return parseOpdsFeed(await response.text())
}
