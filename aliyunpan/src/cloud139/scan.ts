import type { IAliGetFileModel } from '../aliapi/alimodels'
import { apiCloud139FileListPage, mapCloud139FileToAliModel } from './dirfilelist'

export async function* iterateCloud139FolderPages(folder: IAliGetFileModel, userId: string, driveId: string, shouldStop?: () => boolean): AsyncGenerator<IAliGetFileModel[]> {
  const parentId = folder.file_id || 'cloud139_root'
  let cursor = ''
  const seenCursors = new Set<string>()
  do {
    if (seenCursors.has(cursor)) return
    seenCursors.add(cursor)
    const page = await apiCloud139FileListPage(userId, parentId, 100, cursor)
    if (page.items.length) yield page.items.map(item => ({ ...mapCloud139FileToAliModel(item, driveId, parentId), user_id: userId } as IAliGetFileModel))
    cursor = page.nextCursor
  } while (cursor && !shouldStop?.())
}
