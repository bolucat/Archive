export type LibrarySourceKind = 'music' | 'book'

export interface ILibrarySource {
  id: string
  kind: LibrarySourceKind
  user_id: string
  drive_id: string
  folder_id: string
  name: string
  path?: string
  created_at: number
  scanned_at: number
  item_count?: number
}

export function buildLibrarySourceId(kind: LibrarySourceKind, userId: string, driveId: string, folderId: string): string {
  return `${kind}|${userId}|${driveId}|${folderId}`
}
