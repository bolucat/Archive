export interface ReaderReferPositionRect {
  left: number
  top: number
  width: number
  height?: number
  bottom: number
}

export interface ReaderReferViewport {
  width: number
  height: number
}

const REFER_POPUP_WIDTH = 290
const REFER_POPUP_HEIGHT = 250
const REFER_MARGIN = 20

export function stripReferHtml(html: string): string {
  return decodeReaderReferEntities(html || '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
}

export function sanitizeReferHtml(html: string): string {
  return (html || '')
    .replace(/<\s*(script|style|iframe|object|embed)\b[^>]*>[\s\S]*?<\s*\/\s*\1\s*>/gi, '')
    .replace(/<\s*(script|style|iframe|object|embed)\b[^>]*\/?\s*>/gi, '')
    .replace(/\s+on[a-z]+\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+)/gi, '')
    .replace(/\s+(href|src)\s*=\s*(["'])\s*javascript:[\s\S]*?\2/gi, ' $1="#"')
    .replace(/\s+(href|src)\s*=\s*javascript:[^\s>]+/gi, ' $1="#"')
}

function decodeReaderReferEntities(text: string): string {
  return text
    .replace(/&nbsp;/gi, ' ')
    .replace(/&amp;/gi, '&')
    .replace(/&lt;/gi, '<')
    .replace(/&gt;/gi, '>')
    .replace(/&quot;/gi, '"')
    .replace(/&#39;/g, "'")
}

export function buildReferPopupPosition(
  targetRect: ReaderReferPositionRect,
  iframeRect: Pick<ReaderReferPositionRect, 'left' | 'top'>,
  viewport: ReaderReferViewport
): { x: number; y: number } {
  const rawX = iframeRect.left + targetRect.left + targetRect.width / 2 - REFER_POPUP_WIDTH / 2
  const rawY = iframeRect.top + targetRect.bottom + 50
  const maxX = Math.max(REFER_MARGIN, viewport.width - REFER_POPUP_WIDTH - REFER_MARGIN)
  const maxY = Math.max(REFER_MARGIN, viewport.height - REFER_POPUP_HEIGHT - REFER_MARGIN)
  return {
    x: Math.max(REFER_MARGIN, Math.min(rawX, maxX)),
    y: Math.max(REFER_MARGIN, Math.min(rawY, maxY))
  }
}
