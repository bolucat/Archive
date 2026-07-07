import { isReadableBookFormat } from './bookReaderCapabilities'

export type ManagerHeaderActionKey =
  | 'import-local'
  | 'scan-cloud'
  | 'opds'
  | 'sort'
  | 'view-mode'
  | 'settings'

export interface ManagerHeaderAction {
  key: ManagerHeaderActionKey
  label: string
  title: string
}

const HEADER_ACTIONS: ManagerHeaderAction[] = [
  { key: 'import-local', label: 'Import', title: 'Import local books' },
  { key: 'scan-cloud', label: 'Scan', title: 'Scan cloud drive books' },
  { key: 'opds', label: 'OPDS', title: 'Import from OPDS catalog' },
  { key: 'sort', label: 'Sort', title: 'Sort books and notes' },
  { key: 'view-mode', label: 'View', title: 'Switch view mode' },
  { key: 'settings', label: 'Settings', title: 'Book manager settings' },
]

export function getManagerHeaderActions(): ManagerHeaderAction[] {
  return [...HEADER_ACTIONS]
}

export function buildKoodoNoteDeepLink(noteId: string): string {
  return `koodo-reader://open-note?noteKey=${encodeURIComponent(noteId)}`
}

export function isKoodoImportableFile(fileName: string): boolean {
  const ext = fileName.split('.').pop() || ''
  return isReadableBookFormat(ext)
}

export function summarizeDroppedFiles(fileNames: string[]): { supported: string[]; rejected: string[] } {
  const supported: string[] = []
  const rejected: string[] = []

  for (const fileName of fileNames) {
    if (isKoodoImportableFile(fileName)) supported.push(fileName)
    else rejected.push(fileName)
  }

  return { supported, rejected }
}

export function validateOpdsUrl(input: string): { ok: true; url: string } | { ok: false; error: string } {
  try {
    const url = new URL(input.trim())
    if (url.protocol !== 'http:' && url.protocol !== 'https:') {
      return { ok: false, error: 'Only http and https OPDS urls are supported' }
    }
    return { ok: true, url: url.toString() }
  } catch {
    return { ok: false, error: 'Invalid OPDS url' }
  }
}

export function shouldShowProtectionOverlay(state: { enabled: boolean; unlocked: boolean }): boolean {
  return state.enabled && !state.unlocked
}
