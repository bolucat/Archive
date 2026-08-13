export const DOCUMENT_INSIGHT_EXTENSIONS = ['.pdf', '.docx', '.epub', '.txt', '.md', '.markdown'] as const
export const MAX_DOCUMENT_INSIGHT_SOURCES = 10

export interface DocumentInsightSourceInput {
  file: any
  userId: string
}

export interface DocumentInsightLaunchContext {
  sources: DocumentInsightSourceInput[]
  availableSources?: DocumentInsightSourceInput[]
  scopeName?: string
  initialPrompt?: string
}

export function isDocumentInsightFile(file: any): boolean {
  if (!file || file.isDir) return false
  const name = String(file.name || file.file_name || '').toLowerCase()
  return DOCUMENT_INSIGHT_EXTENSIONS.some(extension => name.endsWith(extension))
}

export function toDocumentInsightSource(file: any, fallbackUserId: string): DocumentInsightSourceInput | null {
  if (!isDocumentInsightFile(file) || !file?.file_id) return null
  return { file, userId: String(file.user_id || fallbackUserId || '') }
}

export function documentInsightSourceId(source: DocumentInsightSourceInput): string {
  const file = source.file || {}
  const version = file.content_hash || file.etag || `${file.size || 0}:${file.updated_at || file.time || ''}`
  return `document:${source.userId}:${file.drive_id || ''}:${file.file_id || ''}:${version}`
}

/** Renderer-local launch hand-off; it contains no download URL, text, or token. */
export function openDocumentInsight(context: DocumentInsightLaunchContext): void {
  const sources = context.sources.filter(Boolean).slice(0, MAX_DOCUMENT_INSIGHT_SOURCES)
  if (!sources.length) return
  sessionStorage.setItem('boxplayer:pending-document-ai', JSON.stringify({ ...context, sources }))
  window.dispatchEvent(new CustomEvent('boxplayer:open-document-ai'))
}
