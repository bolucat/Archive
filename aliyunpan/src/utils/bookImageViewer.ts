export interface BookImagePreview {
  src: string
  name: string
  ratio: 'horizontal' | 'vertical'
}

const IMAGE_EXTENSION_BY_TYPE: Record<string, string> = {
  'image/jpeg': 'jpg',
  'image/jpg': 'jpg',
  'image/png': 'png',
  'image/gif': 'gif',
  'image/webp': 'webp',
  'image/svg+xml': 'svg',
  'image/bmp': 'bmp'
}

export function normalizeBookImageSource(src: string, baseUrl = ''): string {
  const value = (src || '').trim()
  if (!value) return ''
  if (/^(https?:|data:|blob:)/i.test(value)) return value
  try {
    return baseUrl ? new URL(value, baseUrl).toString() : value
  } catch {
    return value
  }
}

export function shouldPreviewBookImage(options: {
  tagName: string
  src?: string
  href?: string
  alt?: string
  className?: string
  id?: string
}): boolean {
  const tag = options.tagName.toLowerCase()
  if (tag !== 'img' && tag !== 'image') return false
  if (!options.src?.trim()) return false
  const href = options.href || ''
  if (href && (href.includes('#') || href.startsWith('../') || /^https?:|^mailto:|^kindle:/i.test(href))) return false
  if (options.alt && `${options.className || ''} ${options.id || ''}`.toLowerCase().includes('footnote')) return false
  return true
}

export function getBookImageRatio(width: number, height: number): 'horizontal' | 'vertical' {
  return width > height ? 'horizontal' : 'vertical'
}

export function getBookImageScaleStyle(ratio: 'horizontal' | 'vertical', zoomIndex: number): Record<string, string> {
  return ratio === 'horizontal'
    ? { width: `${Math.max(10, Math.min(200, 60 + zoomIndex * 10))}vw` }
    : { height: `${Math.max(10, Math.min(200, 100 + zoomIndex * 10))}vh` }
}

export function getBookImageTransform(rotateIndex: number): string {
  return `rotate(${rotateIndex * 90}deg)`
}

export function buildReaderImageFileName(name: string, mimeType = '', now = new Date()): string {
  const cleanName = name.trim().replace(/[<>:"/\\|?*\u0000-\u001F]/g, '-')
  const baseName = cleanName || now.toLocaleDateString()
  if (/\.[a-z0-9]{2,5}$/i.test(baseName)) return baseName
  const extension = IMAGE_EXTENSION_BY_TYPE[mimeType.toLowerCase()]
  return extension ? `${baseName}.${extension}` : baseName
}

export async function fetchReaderImageBlob(src: string, fetcher: typeof fetch = fetch): Promise<Blob> {
  const response = await fetcher(src)
  if (!response.ok) throw new Error(`Failed to fetch image: ${response.status}`)
  return response.blob()
}

export async function downloadBookImage(
  src: string,
  name: string,
  deps: {
    fetcher?: typeof fetch
    document?: Document
    url?: Pick<typeof URL, 'createObjectURL' | 'revokeObjectURL'>
    setTimeout?: (handler: () => void, timeout?: number) => unknown
    now?: Date
  } = {}
): Promise<string> {
  const blob = await fetchReaderImageBlob(src, deps.fetcher)
  const filename = buildReaderImageFileName(name, blob.type, deps.now)
  const doc = deps.document || document
  const urlApi = deps.url || URL
  const defer = deps.setTimeout || window.setTimeout.bind(window)
  const objectUrl = urlApi.createObjectURL(blob)
  let link: HTMLAnchorElement | null = null
  try {
    link = doc.createElement('a')
    link.href = objectUrl
    link.download = filename
    link.rel = 'noopener'
    link.style.display = 'none'
    doc.body?.appendChild(link)
    link.click()
  } finally {
    link?.remove()
    defer(() => urlApi.revokeObjectURL(objectUrl), 0)
  }
  return filename
}

export async function copyBookImageToClipboard(
  src: string,
  deps: {
    fetcher?: typeof fetch
    createImage?: () => HTMLImageElement
    document?: Document
    url?: Pick<typeof URL, 'createObjectURL' | 'revokeObjectURL'>
    clipboard?: Pick<Clipboard, 'write'>
    ClipboardItemCtor?: typeof ClipboardItem
  } = {}
): Promise<void> {
  const clipboard = deps.clipboard || navigator.clipboard
  const ClipboardItemCtor =
    deps.ClipboardItemCtor || (globalThis as unknown as { ClipboardItem?: typeof ClipboardItem }).ClipboardItem
  if (!clipboard?.write || !ClipboardItemCtor) throw new Error('Image clipboard is unavailable')

  const blob = await fetchReaderImageBlob(src, deps.fetcher)
  const doc = deps.document || document
  const urlApi = deps.url || URL
  const objectUrl = urlApi.createObjectURL(blob)
  try {
    const img = deps.createImage ? deps.createImage() : new Image()
    await new Promise<void>((resolve, reject) => {
      img.onload = () => resolve()
      img.onerror = () => reject(new Error('Failed to load image'))
      img.src = objectUrl
    })

    const canvas = doc.createElement('canvas')
    canvas.width = img.naturalWidth
    canvas.height = img.naturalHeight
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('Canvas is unavailable')
    ctx.drawImage(img, 0, 0)

    const pngBlob = await new Promise<Blob>((resolve, reject) => {
      canvas.toBlob((value) => {
        if (value) resolve(value)
        else reject(new Error('Failed to convert image'))
      }, 'image/png')
    })
    await clipboard.write([new ClipboardItemCtor({ 'image/png': pngBlob })])
  } finally {
    urlApi.revokeObjectURL(objectUrl)
  }
}
