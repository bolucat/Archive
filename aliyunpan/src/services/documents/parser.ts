import JSZip from 'jszip'
import pdfWorkerUrl from 'pdfjs-dist/build/pdf.worker.js?url'

export const MAX_DOCUMENT_BYTES = 50 * 1024 * 1024
export const MAX_DOCUMENT_CHARS = 2_000_000

export interface ParsedDocumentSection {
  index: number
  title: string
  text: string
  location: string
}

export interface ParsedDocument {
  fileName: string
  sections: ParsedDocumentSection[]
  totalChars: number
}

function extensionOf(fileName: string): string {
  return fileName.toLowerCase().split('.').pop() || ''
}

function normalizeText(text: string): string {
  return text.replace(/\r\n?/g, '\n').replace(/\n{3,}/g, '\n\n').trim()
}

function enforceLimits(fileName: string, data: ArrayBuffer, sections: ParsedDocumentSection[]): ParsedDocument {
  if (data.byteLength > MAX_DOCUMENT_BYTES) throw new Error('document_too_large')
  const totalChars = sections.reduce((sum, section) => sum + section.text.length, 0)
  if (totalChars > MAX_DOCUMENT_CHARS) throw new Error('document_text_too_large')
  if (!totalChars) throw new Error('document_has_no_text')
  return { fileName, sections, totalChars }
}

async function parsePdf(data: ArrayBuffer): Promise<ParsedDocumentSection[]> {
  const pdfjs = await import('pdfjs-dist') as any
  // Keep document analysis on the same Vite-managed PDF.js worker path as the
  // PDF viewer. Without this, pdfjs-dist tries to infer a worker URL at runtime
  // and fails in Electron with "No GlobalWorkerOptions.workerSrc specified".
  pdfjs.GlobalWorkerOptions.workerSrc = pdfWorkerUrl
  const document = await pdfjs.getDocument({ data: new Uint8Array(data) }).promise
  const sections: ParsedDocumentSection[] = []
  for (let pageNumber = 1; pageNumber <= document.numPages; pageNumber++) {
    const page = await document.getPage(pageNumber)
    const content = await page.getTextContent()
    const text = normalizeText(content.items.map((item: any) => item.str || '').join(' '))
    if (text) sections.push({ index: pageNumber - 1, title: `第 ${pageNumber} 页`, text, location: `page:${pageNumber}` })
  }
  return sections
}

async function parseDocx(data: ArrayBuffer): Promise<ParsedDocumentSection[]> {
  const mammothModule = await import('mammoth') as any
  const mammoth = mammothModule.default || mammothModule
  const result = await mammoth.extractRawText({ arrayBuffer: data })
  const text = normalizeText(result.value || '')
  return text ? [{ index: 0, title: '正文', text, location: 'document:body' }] : []
}

function stripHtml(html: string): string {
  if (typeof DOMParser !== 'undefined') {
    return normalizeText(new DOMParser().parseFromString(html, 'text/html').body.textContent || '')
  }
  return normalizeText(html.replace(/<script[\s\S]*?<\/script>/gi, ' ').replace(/<style[\s\S]*?<\/style>/gi, ' ').replace(/<[^>]+>/g, ' '))
}

function parseAttributes(tag: string): Record<string, string> {
  const attrs: Record<string, string> = {}
  for (const match of tag.matchAll(/([\w:-]+)\s*=\s*["']([^"']*)["']/g)) attrs[match[1]] = match[2]
  return attrs
}

async function parseEpub(data: ArrayBuffer): Promise<ParsedDocumentSection[]> {
  const zip = await JSZip.loadAsync(data)
  const container = await zip.file('META-INF/container.xml')?.async('text')
  const opfPath = container?.match(/rootfile[^>]+full-path=["']([^"']+)["']/i)?.[1]
  if (!opfPath) throw new Error('invalid_epub')
  const opf = await zip.file(opfPath)?.async('text')
  if (!opf) throw new Error('invalid_epub')

  const manifest = new Map<string, string>()
  for (const match of opf.matchAll(/<item\b[^>]*>/gi)) {
    const attrs = parseAttributes(match[0])
    if (attrs.id && attrs.href) manifest.set(attrs.id, attrs.href)
  }
  const spine = [...opf.matchAll(/<itemref\b[^>]*>/gi)].map(match => parseAttributes(match[0]).idref).filter(Boolean)
  const basePath = opfPath.includes('/') ? opfPath.slice(0, opfPath.lastIndexOf('/') + 1) : ''
  const sections: ParsedDocumentSection[] = []
  for (const id of spine) {
    const href = manifest.get(id)
    if (!href) continue
    const html = await zip.file(`${basePath}${decodeURIComponent(href.split('#')[0])}`)?.async('text')
    const text = html ? stripHtml(html) : ''
    if (text) sections.push({ index: sections.length, title: `章节 ${sections.length + 1}`, text, location: `epub:${href}` })
  }
  return sections
}

export async function parseDocument(fileName: string, data: ArrayBuffer): Promise<ParsedDocument> {
  if (data.byteLength > MAX_DOCUMENT_BYTES) throw new Error('document_too_large')
  const extension = extensionOf(fileName)
  let sections: ParsedDocumentSection[]
  if (extension === 'txt' || extension === 'md' || extension === 'markdown') {
    const text = normalizeText(new TextDecoder().decode(data))
    sections = text ? [{ index: 0, title: '正文', text, location: 'document:body' }] : []
  } else if (extension === 'pdf') {
    sections = await parsePdf(data)
  } else if (extension === 'docx') {
    sections = await parseDocx(data)
  } else if (extension === 'epub') {
    sections = await parseEpub(data)
  } else {
    throw new Error('unsupported_document_type')
  }
  return enforceLimits(fileName, data, sections)
}
