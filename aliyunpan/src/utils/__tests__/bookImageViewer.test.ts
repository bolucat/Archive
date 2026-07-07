import { describe, expect, it } from 'vitest'
import {
  buildReaderImageFileName,
  copyBookImageToClipboard,
  downloadBookImage,
  getBookImageRatio,
  getBookImageScaleStyle,
  getBookImageTransform,
  normalizeBookImageSource,
  shouldPreviewBookImage
} from '../bookImageViewer'

describe('readerImageViewer', () => {
  it('resolves relative image sources against the iframe document URL', () => {
    expect(normalizeBookImageSource('../images/cover.jpg', 'https://book.test/OEBPS/chapter.xhtml'))
      .toBe('https://book.test/images/cover.jpg')
    expect(normalizeBookImageSource('data:image/png;base64,abc', 'https://book.test/a.xhtml'))
      .toBe('data:image/png;base64,abc')
  })

  it('previews only standalone image targets', () => {
    expect(shouldPreviewBookImage({ tagName: 'IMG', src: 'cover.jpg' })).toBe(true)
    expect(shouldPreviewBookImage({ tagName: 'A', src: 'cover.jpg' })).toBe(false)
    expect(shouldPreviewBookImage({ tagName: 'IMG', src: 'cover.jpg', href: '#note' })).toBe(false)
    expect(shouldPreviewBookImage({ tagName: 'IMG', src: 'cover.jpg', alt: 'note', className: 'footnote-ref' })).toBe(false)
  })

  it('matches Reader image ratio, zoom and rotation rules', () => {
    expect(getBookImageRatio(1200, 800)).toBe('horizontal')
    expect(getBookImageRatio(800, 1200)).toBe('vertical')
    expect(getBookImageScaleStyle('horizontal', 2)).toEqual({ width: '80vw' })
    expect(getBookImageScaleStyle('vertical', -20)).toEqual({ height: '10vh' })
    expect(getBookImageTransform(3)).toBe('rotate(270deg)')
  })

  it('builds a safe download filename from alt text and image type', () => {
    expect(buildReaderImageFileName('cover:name', 'image/png')).toBe('cover-name.png')
    expect(buildReaderImageFileName('cover.jpg', 'image/png')).toBe('cover.jpg')
    expect(buildReaderImageFileName('', 'image/jpeg', new Date('2026-06-03T00:00:00Z'))).toMatch(/jpg$/)
  })

  it('downloads the image through an object url', async () => {
    const clicked: string[] = []
    const revoked: string[] = []
    const appended: unknown[] = []
    const removed: unknown[] = []
    const timers: Array<() => void> = []
    const link = {
      href: '',
      download: '',
      rel: '',
      style: { display: '' },
      click: () => clicked.push(link.href),
      remove: () => removed.push(link)
    }
    const fetcher = async () => new Response(new Blob(['image'], { type: 'image/webp' }))
    const filename = await downloadBookImage('blob:cover', 'cover', {
      fetcher,
      document: {
        body: { appendChild: (item: unknown) => appended.push(item) },
        createElement: () => link
      } as unknown as Document,
      url: {
        createObjectURL: () => 'blob:download',
        revokeObjectURL: (url) => revoked.push(url)
      },
      setTimeout: (handler) => timers.push(handler)
    })

    expect(filename).toBe('cover.webp')
    expect(link.download).toBe('cover.webp')
    expect(link.style.display).toBe('none')
    expect(appended).toEqual([link])
    expect(removed).toEqual([link])
    expect(clicked).toEqual(['blob:download'])
    expect(revoked).toEqual([])
    timers.forEach((run) => run())
    expect(revoked).toEqual(['blob:download'])
  })

  it('reports image clipboard as unavailable without touching the image source', async () => {
    const originalClipboardItem = (globalThis as { ClipboardItem?: typeof ClipboardItem }).ClipboardItem
    delete (globalThis as { ClipboardItem?: typeof ClipboardItem }).ClipboardItem
    let fetched = false
    try {
      await expect(copyBookImageToClipboard('blob:cover', {
        fetcher: async () => {
          fetched = true
          return new Response(new Blob(['image'], { type: 'image/jpeg' }))
        },
        clipboard: { write: async () => {} } as unknown as Clipboard
      })).rejects.toThrow('Image clipboard is unavailable')
    } finally {
      ;(globalThis as { ClipboardItem?: typeof ClipboardItem }).ClipboardItem = originalClipboardItem
    }
    expect(fetched).toBe(false)
  })

  it('copies the image as png to the clipboard', async () => {
    const written: unknown[] = []
    const revoked: string[] = []
    const png = new Blob(['png'], { type: 'image/png' })
    class FakeClipboardItem {
      constructor(public readonly data: Record<string, Blob>) {}
    }
    const img = {
      naturalWidth: 12,
      naturalHeight: 8,
      onload: null as null | ((event: Event) => void),
      onerror: null as null | ((event: Event) => void),
      set src(_value: string) {
        this.onload?.({} as Event)
      }
    } as HTMLImageElement
    const canvas = {
      width: 0,
      height: 0,
      getContext: () => ({ drawImage: () => {} }),
      toBlob: (callback: (value: Blob | null) => void) => callback(png)
    }

    await copyBookImageToClipboard('blob:cover', {
      fetcher: async () => new Response(new Blob(['image'], { type: 'image/jpeg' })),
      createImage: () => img,
      document: { createElement: () => canvas } as unknown as Document,
      url: {
        createObjectURL: () => 'blob:image',
        revokeObjectURL: (url) => revoked.push(url)
      },
      clipboard: { write: async (items: ClipboardItems) => { written.push(...items) } } as unknown as Clipboard,
      ClipboardItemCtor: FakeClipboardItem as unknown as typeof ClipboardItem
    })

    expect(canvas.width).toBe(12)
    expect(canvas.height).toBe(8)
    expect(written).toHaveLength(1)
    expect((written[0] as FakeClipboardItem).data['image/png']).toBe(png)
    expect(revoked).toEqual(['blob:image'])
  })

  it('releases the object url when image loading fails during copy', async () => {
    const revoked: string[] = []
    class FakeClipboardItem {
      constructor(public readonly data: Record<string, Blob>) {}
    }
    const img = {
      naturalWidth: 0,
      naturalHeight: 0,
      onload: null as null | ((event: Event) => void),
      onerror: null as null | ((event: Event) => void),
      set src(_value: string) {
        this.onerror?.({} as Event)
      }
    } as HTMLImageElement

    await expect(copyBookImageToClipboard('blob:cover', {
      fetcher: async () => new Response(new Blob(['image'], { type: 'image/jpeg' })),
      createImage: () => img,
      document: {} as Document,
      url: {
        createObjectURL: () => 'blob:image',
        revokeObjectURL: (url) => revoked.push(url)
      },
      clipboard: { write: async () => {} } as unknown as Clipboard,
      ClipboardItemCtor: FakeClipboardItem as unknown as typeof ClipboardItem
    })).rejects.toThrow('Failed to load image')

    expect(revoked).toEqual(['blob:image'])
  })
})
