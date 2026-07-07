import type { IBookItem } from '../types/book'
import type { IBookBookmark } from '../types/bookBookmark'
import type { IBookNote } from '../types/bookNote'
import { buildNoteLink } from './bookManagerParity'

export type AnnotationExportFormat = 'md' | 'txt' | 'html' | 'csv'

interface ExportBookAnnotationsOptions {
  book: Pick<IBookItem, 'title' | 'file_name' | 'author'>
  notes: IBookNote[]
  bookmarks: IBookBookmark[]
  exportedAt?: Date
}

interface ExportAllAnnotationsOptions {
  books: Pick<IBookItem, 'id' | 'title' | 'file_name' | 'author'>[]
  notes: IBookNote[]
  bookmarks: IBookBookmark[]
  exportedAt?: Date
}

function cleanText(value: string): string {
  return (value || '').replace(/\r\n/g, '\n').trim()
}

function escapeFileName(value: string): string {
  return cleanText(value).replace(/[\\/:*?"<>|]+/g, '_').replace(/\s+/g, ' ').slice(0, 80) || 'book'
}

function htmlEscape(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

function csvEscape(value: string): string {
  if (!value) return ''
  const escaped = value.replace(/"/g, '""').replace(/\n/g, ' ')
  return `"${escaped}"`
}

function formatPercent(value?: number): string {
  if (value === undefined || !Number.isFinite(value)) return '-'
  return `${Math.max(0, Math.min(100, Math.round(value * 100)))}%`
}

function formatDate(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`
}

export function buildBookAnnotationsMarkdown(options: ExportBookAnnotationsOptions): string {
  const title = cleanText(options.book.title || options.book.file_name || '未命名书籍')
  const author = cleanText(options.book.author || '')
  const exportedAt = options.exportedAt || new Date()
  const lines = [
    `# ${title}`,
    '',
    author ? `作者：${author}` : '',
    `导出时间：${formatDate(exportedAt)}`,
    ''
  ].filter((line, index) => index < 2 || line)

  const notes = [...options.notes].sort((a, b) => (a.position?.percentage || 0) - (b.position?.percentage || 0))
  lines.push('## 书摘', '')
  if (!notes.length) {
    lines.push('暂无书摘', '')
  } else {
    for (const note of notes) {
      lines.push(`### ${note.chapter || '未知章节'} · ${formatPercent(note.position?.percentage)}`)
      lines.push('')
      lines.push(`> ${cleanText(note.text).replace(/\n/g, '\n> ')}`)
      if (note.note) {
        lines.push('', `备注：${cleanText(note.note)}`)
      }
      if (note.tags?.length) {
        lines.push('', `标签：${note.tags.join(', ')}`)
      }
      lines.push('')
    }
  }

  const bookmarks = [...options.bookmarks].sort((a, b) => a.percentage - b.percentage)
  lines.push('## 书签', '')
  if (!bookmarks.length) {
    lines.push('暂无书签', '')
  } else {
    for (const bookmark of bookmarks) {
      lines.push(`- ${formatPercent(bookmark.percentage)} · ${bookmark.chapter || '未知章节'} · ${cleanText(bookmark.label || '书签')}`)
    }
    lines.push('')
  }

  return `${lines.join('\n').replace(/\n{3,}/g, '\n\n').trim()}\n`
}

export function buildBookAnnotationsHTML(options: ExportBookAnnotationsOptions): string {
  const title = htmlEscape(cleanText(options.book.title || options.book.file_name || '未命名书籍'))
  const author = htmlEscape(cleanText(options.book.author || ''))
  const exportedAt = formatDate(options.exportedAt || new Date())
  const css = `
    body { font-family: -apple-system, system-ui, sans-serif; max-width: 720px; margin: 32px auto; padding: 0 20px; color: #333; line-height: 1.6; }
    h1 { font-size: 24px; margin-bottom: 8px; }
    .meta { color: #888; font-size: 13px; margin-bottom: 24px; }
    h2 { font-size: 18px; border-bottom: 1px solid #eee; padding-bottom: 6px; margin-top: 24px; }
    h3 { font-size: 15px; margin: 18px 0 6px; color: #555; }
    blockquote { border-left: 3px solid #4a90d9; margin: 8px 0; padding: 6px 12px; background: #f8f8f8; }
    .note-tags { font-size: 12px; color: #4a90d9; margin-top: 4px; }
    .note-note { font-size: 13px; color: #555; margin-top: 6px; }
    .bookmark-item { margin: 4px 0; font-size: 14px; }
    ul { padding-left: 20px; }`

  const notes = [...options.notes].sort((a, b) => (a.position?.percentage || 0) - (b.position?.percentage || 0))
  let notesHtml = ''
  if (!notes.length) {
    notesHtml = '<p><em>暂无书摘</em></p>\n'
  } else {
    for (const note of notes) {
      notesHtml += `<h3>${htmlEscape(note.chapter || '未知章节')} · ${formatPercent(note.position?.percentage)}</h3>\n`
      notesHtml += `<blockquote>${htmlEscape(cleanText(note.text)).replace(/\n/g, '<br>\n')}</blockquote>\n`
      if (note.note) {
        notesHtml += `<p class="note-note">备注：${htmlEscape(cleanText(note.note))}</p>\n`
      }
      if (note.tags?.length) {
        notesHtml += `<p class="note-tags">标签：${htmlEscape(note.tags.join(', '))}</p>\n`
      }
    }
  }

  let bookmarksHtml = ''
  const bookmarks = [...options.bookmarks].sort((a, b) => a.percentage - b.percentage)
  if (!bookmarks.length) {
    bookmarksHtml = '<p><em>暂无书签</em></p>\n'
  } else {
    bookmarksHtml += '<ul>\n'
    for (const bookmark of bookmarks) {
      bookmarksHtml += `  <li class="bookmark-item">${formatPercent(bookmark.percentage)} · ${htmlEscape(bookmark.chapter || '未知章节')} · ${htmlEscape(cleanText(bookmark.label || '书签'))}</li>\n`
    }
    bookmarksHtml += '</ul>\n'
  }

  return `<!DOCTYPE html>
<html lang="zh-CN">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${title} - 书摘</title><style>${css}</style></head>
<body>
<h1>${title}</h1>
<div class="meta">${author ? `作者：${author}<br>` : ''}导出时间：${exportedAt}</div>
<h2>书摘</h2>
${notesHtml}
<h2>书签</h2>
${bookmarksHtml}
</body>
</html>`
}

export function buildBookAnnotationsTXT(options: ExportBookAnnotationsOptions): string {
  const title = cleanText(options.book.title || options.book.file_name || '未命名书籍')
  const author = cleanText(options.book.author || '')
  const exportedAt = formatDate(options.exportedAt || new Date())
  const lines = [
    title,
    '='.repeat(Math.min(40, title.length)),
    '',
    author ? `作者：${author}` : '',
    `导出时间：${exportedAt}`,
    ''
  ].filter((line, index) => index < 4 || line)

  const notes = [...options.notes].sort((a, b) => (a.position?.percentage || 0) - (b.position?.percentage || 0))
  lines.push('--- 书摘 ---', '')
  if (!notes.length) {
    lines.push('暂无书摘', '')
  } else {
    for (const note of notes) {
      lines.push(`[${note.chapter || '未知章节'}] ${formatPercent(note.position?.percentage)}`)
      lines.push(cleanText(note.text).split('\n').map((l) => `  ${l}`).join('\n'))
      if (note.note) {
        lines.push('', `  备注：${cleanText(note.note)}`)
      }
      if (note.tags?.length) {
        lines.push('', `  标签：${note.tags.join(', ')}`)
      }
      lines.push('')
    }
  }

  const bookmarks = [...options.bookmarks].sort((a, b) => a.percentage - b.percentage)
  lines.push('--- 书签 ---', '')
  if (!bookmarks.length) {
    lines.push('暂无书签', '')
  } else {
    for (const bookmark of bookmarks) {
      lines.push(`  ${formatPercent(bookmark.percentage)} · ${bookmark.chapter || '未知章节'} · ${cleanText(bookmark.label || '书签')}`)
    }
    lines.push('')
  }

  return lines.join('\n')
}

export function buildBookAnnotationsCSV(options: ExportBookAnnotationsOptions): string {
  const headers = ['类型', '章节', '进度', '摘录', '备注', '标签']
  const lines = [headers.map(csvEscape).join(',')]

  const notes = [...options.notes].sort((a, b) => (a.position?.percentage || 0) - (b.position?.percentage || 0))
  for (const note of notes) {
    lines.push([
      csvEscape(note.kind === 'note' ? '笔记' : '高亮'),
      csvEscape(note.chapter || '未知章节'),
      formatPercent(note.position?.percentage),
      csvEscape(cleanText(note.text)),
      csvEscape(note.note || ''),
      csvEscape((note.tags || []).join(', '))
    ].join(','))
  }

  const bookmarks = [...options.bookmarks].sort((a, b) => a.percentage - b.percentage)
  for (const bookmark of bookmarks) {
    lines.push([
      csvEscape('书签'),
      csvEscape(bookmark.chapter || '未知章节'),
      `${Math.round((Number(bookmark.percentage) || 0) * 100)}%`,
      csvEscape(cleanText(bookmark.label || '书签')),
      '',
      ''
    ].join(','))
  }

  return `${lines.join('\n')}\n`
}

export function buildBookAnnotationsFileName(book: Pick<IBookItem, 'title' | 'file_name'>, format: AnnotationExportFormat = 'md'): string {
  const extMap: Record<AnnotationExportFormat, string> = { md: '.md', txt: '.txt', html: '.html', csv: '.csv' }
  const ext = extMap[format] || '.md'
  return `${escapeFileName(book.title || book.file_name || 'book')}-书摘${ext}`
}

export function getExportMimeType(format: AnnotationExportFormat): string {
  const mime: Record<AnnotationExportFormat, string> = {
    md: 'text/markdown;charset=utf-8',
    txt: 'text/plain;charset=utf-8',
    html: 'text/html;charset=utf-8',
    csv: 'text/csv;charset=utf-8'
  }
  return mime[format] || mime.md
}

function buildAnnotationsForFormat(options: ExportBookAnnotationsOptions, format: AnnotationExportFormat): string {
  switch (format) {
    case 'html': return buildBookAnnotationsHTML(options)
    case 'txt': return buildBookAnnotationsTXT(options)
    case 'csv': return buildBookAnnotationsCSV(options)
    default: return buildBookAnnotationsMarkdown(options)
  }
}

export function downloadTextFile(fileName: string, text: string, type = 'text/markdown;charset=utf-8') {
  const blob = new Blob([text], { type })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = fileName
  link.click()
  URL.revokeObjectURL(url)
}

export function exportAnnotations(
  book: Pick<IBookItem, 'title' | 'file_name' | 'author'>,
  notes: IBookNote[],
  bookmarks: IBookBookmark[],
  format: AnnotationExportFormat = 'md'
) {
  const text = buildAnnotationsForFormat({ book, notes, bookmarks }, format)
  const fileName = buildBookAnnotationsFileName(book, format)
  downloadTextFile(fileName, text, getExportMimeType(format))
}

export function exportAllAnnotations(options: ExportAllAnnotationsOptions, format: AnnotationExportFormat = 'md') {
  const bookMap = new Map(options.books.map((b) => [b.id, b]))
  if (format === 'csv') {
    const lines: string[] = []
    const headers = ['书名', '类型', '章节', '进度', '摘录', '备注', '标签']
    lines.push(headers.map(csvEscape).join(','))

    const notes = [...options.notes].sort((a, b) => (b.updated_at || 0) - (a.updated_at || 0))
    for (const note of notes) {
      const book = bookMap.get(note.book_id)
      const bookName = book?.title || book?.file_name || note.book_id
      lines.push([
        csvEscape(bookName),
        csvEscape(note.kind === 'note' ? '笔记' : '高亮'),
        csvEscape(note.chapter || '未知章节'),
        formatPercent(note.position?.percentage),
        csvEscape(cleanText(note.text)),
        csvEscape(note.note || ''),
        csvEscape((note.tags || []).join(', '))
      ].join(','))
    }

    for (const bookmark of options.bookmarks) {
      const book = bookMap.get(bookmark.book_id)
      const bookName = book?.title || book?.file_name || bookmark.book_id
      lines.push([
        csvEscape(bookName),
        csvEscape('书签'),
        csvEscape(bookmark.chapter || '未知章节'),
        `${Math.round((Number(bookmark.percentage) || 0) * 100)}%`,
        csvEscape(cleanText(bookmark.label || '书签')),
        '',
        ''
      ].join(','))
    }
    const text = `${lines.join('\n')}\n`
    const exportedAt = formatDate(options.exportedAt || new Date())
    const fileName = `全部书摘-${exportedAt.replace(/[: ]/g, '-')}.csv`
    downloadTextFile(fileName, text, 'text/csv;charset=utf-8')
    return
  }

  const allTextParts: string[] = []
  const exportedAt = formatDate(options.exportedAt || new Date())

  for (const book of options.books) {
    const bookNotes = options.notes.filter((n) => n.book_id === book.id)
    const bookBookmarks = options.bookmarks.filter((b) => b.book_id === book.id)
    if (!bookNotes.length && !bookBookmarks.length) continue
    const opts: ExportBookAnnotationsOptions = { book, notes: bookNotes, bookmarks: bookBookmarks, exportedAt: options.exportedAt }
    if (format === 'html') {
      const title = htmlEscape(cleanText(book.title || book.file_name || '未命名书籍'))
      const author = htmlEscape(cleanText(book.author || ''))
      allTextParts.push(`<h2>${title}</h2>`)
      if (author) allTextParts.push(`<p>作者：${author}</p>`)
      const sortedNotes = [...bookNotes].sort((a, b) => (a.position?.percentage || 0) - (b.position?.percentage || 0))
      for (const note of sortedNotes) {
        allTextParts.push(`<h3>${htmlEscape(note.chapter || '未知章节')} · ${formatPercent(note.position?.percentage)}</h3>`)
        allTextParts.push(`<blockquote>${htmlEscape(cleanText(note.text)).replace(/\n/g, '<br>\n')}</blockquote>`)
        if (note.note) allTextParts.push(`<p class="note-note">备注：${htmlEscape(cleanText(note.note))}</p>`)
        if (note.tags?.length) allTextParts.push(`<p class="note-tags">标签：${htmlEscape(note.tags.join(', '))}</p>`)
      }
    } else if (format === 'txt') {
      allTextParts.push(buildBookAnnotationsTXT(opts))
      allTextParts.push('', '===', '')
    } else {
      allTextParts.push(buildBookAnnotationsMarkdown(opts))
      allTextParts.push('', '---', '')
    }
  }

  let finalText: string
  let mimeType: string
  if (format === 'html') {
    const css = `body { font-family: -apple-system, system-ui, sans-serif; max-width: 720px; margin: 32px auto; padding: 0 20px; color: #333; line-height: 1.6; } h1 { font-size: 24px; margin-bottom: 8px; } .meta { color: #888; font-size: 13px; margin-bottom: 24px; } h2 { font-size: 20px; border-bottom: 1px solid #eee; padding-bottom: 6px; margin-top: 28px; } h3 { font-size: 15px; margin: 18px 0 6px; color: #555; } blockquote { border-left: 3px solid #4a90d9; margin: 8px 0; padding: 6px 12px; background: #f8f8f8; } .note-tags { font-size: 12px; color: #4a90d9; margin-top: 4px; } .note-note { font-size: 13px; color: #555; margin-top: 6px; }`
    finalText = `<!DOCTYPE html>
<html lang="zh-CN">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>全部书摘</title><style>${css}</style></head>
<body>
<h1>全部书摘</h1>
<div class="meta">导出时间：${exportedAt}</div>
${allTextParts.join('\n')}
</body>
</html>`
    mimeType = 'text/html;charset=utf-8'
  } else {
    finalText = `${allTextParts.join('\n')}\n`
    mimeType = format === 'txt' ? 'text/plain;charset=utf-8' : 'text/markdown;charset=utf-8'
  }

  const fileName = `全部书摘-${exportedAt.replace(/[: ]/g, '-')}.${format}`
  downloadTextFile(fileName, finalText, mimeType)
}
