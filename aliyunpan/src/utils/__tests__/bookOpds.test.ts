import { describe, expect, it, vi } from 'vitest'
import { fetchOpdsFeed, parseOpdsFeed } from '../bookOpds'

const feed = `<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <title>Example Book</title>
    <author><name>Alice</name></author>
    <link rel="http://opds-spec.org/acquisition" href="https://example.com/book.epub" type="application/epub+zip"/>
  </entry>
</feed>`

describe('bookOpds', () => {
  it('parses OPDS acquisition entries', () => {
    expect(parseOpdsFeed(feed)).toEqual([
      {
        title: 'Example Book',
        author: 'Alice',
        href: 'https://example.com/book.epub',
        mime: 'application/epub+zip',
      },
    ])
  })

  it('ignores entries without acquisition links', () => {
    expect(parseOpdsFeed('<feed xmlns="http://www.w3.org/2005/Atom"><entry><title>No Link</title></entry></feed>')).toEqual([])
  })

  it('falls back to href filename and unknown author', () => {
    expect(parseOpdsFeed('<feed><entry><link rel="acquisition" href="https://example.com/No%20Title.pdf"/></entry></feed>')).toEqual([
      {
        title: 'No Title',
        author: 'Unknown Author',
        href: 'https://example.com/No%20Title.pdf',
        mime: 'application/octet-stream',
      },
    ])
  })

  it('fetches and parses OPDS feeds', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      text: () => Promise.resolve(feed),
    })
    vi.stubGlobal('fetch', fetchMock)

    await expect(fetchOpdsFeed('https://example.com/opds')).resolves.toHaveLength(1)
    expect(fetchMock).toHaveBeenCalledWith('https://example.com/opds', expect.objectContaining({
      headers: expect.objectContaining({ Accept: expect.stringContaining('application/atom+xml') }),
    }))

    vi.unstubAllGlobals()
  })

  it('reports failed OPDS requests', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 500 }))

    await expect(fetchOpdsFeed('https://example.com/opds')).rejects.toThrow('OPDS request failed: 500')

    vi.unstubAllGlobals()
  })
})
