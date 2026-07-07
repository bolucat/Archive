export interface EpubMetadata {
  title?: string
  author?: string
  description?: string
  publisher?: string
  language?: string
  isbn?: string
  subjects?: string[]
  coverDataUrl?: string
}

async function loadJsZip(): Promise<any> {
  return (await import('jszip')).default
}

/** 去除 CDATA 包裹和 XML 实体 */
function cleanXmlText(raw: string): string {
  return raw
    .replace(/<!\[CDATA\[([\s\S]*?)\]\]>/g, '$1')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/\s+/g, ' ')
    .trim()
}

/** 从 XML 中提取标签内容，支持 CDATA、多行、属性 */
function extractTag(xml: string, tagName: string): string | undefined {
  // 匹配多行、多属性的 dc:title / dc:creator 等
  const re = new RegExp(`<${tagName}[^>]*>([\\s\\S]*?)<\\/${tagName}>`, 'i')
  const m = xml.match(re)
  if (!m?.[1]) return undefined
  return cleanXmlText(m[1]) || undefined
}

/** 提取所有匹配的标签内容 */
function extractAllTags(xml: string, tagName: string): string[] {
  const re = new RegExp(`<${tagName}[^>]*>([\\s\\S]*?)<\\/${tagName}>`, 'gi')
  const results: string[] = []
  let m: RegExpExecArray | null
  while ((m = re.exec(xml)) !== null) {
    const val = cleanXmlText(m[1])
    if (val) results.push(val)
  }
  return results
}

/** 从 OPF manifest 中提取 item 的 href，支持多种匹配方式 */
function extractItemHref(xml: string, itemId: string): string | undefined {
  const idAttr = itemId.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const re = new RegExp(`<item[^>]*id="${idAttr}"[^>]*href="([^"]+)"`, 'i')
  const m = xml.match(re)
  return m?.[1]
}

function extractCoverId(xml: string): string | undefined {
  // meta name="cover" content="xxx"
  let m = xml.match(/<meta[^>]*name="cover"[^>]*content="([^"]+)"/i)
  if (m?.[1]) return m[1]
  // meta property="cover" (calibre)
  m = xml.match(/<meta[^>]*property="cover"[^>]*>([^<]+)<\/meta>/i)
  return m?.[1]?.trim()
}

export async function extractEpubMeta(buffer: ArrayBuffer): Promise<EpubMetadata> {
  try {
    const JSZip = await loadJsZip()
    const zip = await JSZip.loadAsync(buffer)
    const meta: EpubMetadata = {}

    // 1. container.xml → OPF path
    const containerFile = zip.file('META-INF/container.xml')
    if (!containerFile) return meta
    const containerXml = await containerFile.async('text')
    const opfMatch = containerXml.match(/full-path="([^"]+\.opf)"/i) || containerXml.match(/full-path='([^']+\.opf)'/i)
    if (!opfMatch?.[1]) return meta
    const opfPath = opfMatch[1]

    // 2. 读取 OPF
    const opfFile = zip.file(opfPath)
    if (!opfFile) return meta
    const opfXml = await opfFile.async('text')

    // 3. 基础元数据 — 支持 dc: 和 opf: 命名空间前缀
    meta.title = extractTag(opfXml, 'dc:title') || extractTag(opfXml, 'opf:title')
    meta.author = extractTag(opfXml, 'dc:creator') || extractTag(opfXml, 'opf:creator')
    meta.description = extractTag(opfXml, 'dc:description') || extractTag(opfXml, 'opf:description')
    meta.publisher = extractTag(opfXml, 'dc:publisher') || extractTag(opfXml, 'opf:publisher')
    meta.language = extractTag(opfXml, 'dc:language') || extractTag(opfXml, 'opf:language')

    // 4. 提取所有 creator (带 role 时区分作者/译者)
    const allCreators = extractAllTags(opfXml, 'dc:creator')
    if (allCreators.length > 0) {
      // 优先找 role="aut" 的
      const autMatch = opfXml.match(/<dc:creator[^>]*opf:role="aut"[^>]*>([\s\S]*?)<\/dc:creator>/i)
      if (autMatch?.[1]) {
        meta.author = cleanXmlText(autMatch[1])
      } else {
        meta.author = allCreators[0] // 第一个 creator 作为默认作者
      }
    }

    // 5. ISBN / identifier
    const identifiers = extractAllTags(opfXml, 'dc:identifier')
    for (const id of identifiers) {
      if (/^(97[89]\d{10}|\d{9}[\dX])$/i.test(id.replace(/-/g, ''))) {
        meta.isbn = id
        break
      }
    }

    // 6. Subjects
    meta.subjects = extractAllTags(opfXml, 'dc:subject').slice(0, 10)

    // 7. 封面图片
    const coverId = extractCoverId(opfXml)
    let coverHref: string | undefined
    if (coverId) coverHref = extractItemHref(opfXml, coverId)

    // fallback: id 或 href 包含 cover
    if (!coverHref) {
      const coverRe = /<item[^>]*\b(?:id|href)="([^"]*cover[^"]*)"[^>]*href="([^"]+)"[^>]*>/i
      const m = opfXml.match(coverRe)
      if (m?.[2]) coverHref = m[2]
    }
    // fallback: 第一个 image/*
    if (!coverHref) {
      const m = opfXml.match(/<item[^>]*media-type="image\/[^"]*"[^>]*href="([^"]+)"[^>]*>/i)
      if (m?.[1]) coverHref = m[1]
    }

    if (coverHref) {
      const opfDir = opfPath.split('/').slice(0, -1).join('/')
      const coverPath = opfDir ? `${opfDir}/${encodeURIComponent(decodeURIComponent(coverHref))}` : coverHref
      // 尝试多种路径变体
      const paths = [coverPath]
      if (opfDir) paths.push(`${opfDir}/${coverHref}`)
      paths.push(coverHref)

      for (const p of paths) {
        const coverFile = zip.file(p)
        if (coverFile) {
          const coverBuffer = await coverFile.async('arraybuffer')
          if (coverBuffer.byteLength > 0 && coverBuffer.byteLength < 10 * 1024 * 1024) {
            const ext = coverHref.split('.').pop()?.toLowerCase() || 'jpeg'
            const mimeMap: Record<string, string> = { png: 'image/png', gif: 'image/gif', webp: 'image/webp', svg: 'image/svg+xml' }
            const mime = mimeMap[ext] || 'image/jpeg'
            const bytes = new Uint8Array(coverBuffer)
            let binary = ''
            for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i])
            meta.coverDataUrl = `data:${mime};base64,${btoa(binary)}`
            break
          }
        }
      }
    }

    return meta
  } catch {
    return {}
  }
}
