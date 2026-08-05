import type { IAliGetFileModel } from '../aliapi/alimodels'
import { apiDropboxFileListPage, mapDropboxFileToAliModel } from './dirfilelist'

export async function* iterateDropboxFolderPages(folder: IAliGetFileModel, userId: string, driveId: string, shouldStop?: () => boolean): AsyncGenerator<IAliGetFileModel[]> {
  const parentId = folder.file_id && !folder.file_id.includes('root') ? folder.file_id : 'dropbox_root'
  let cursor = ''
  const seenCursors = new Set<string>()
  do {
    if (seenCursors.has(cursor)) return
    seenCursors.add(cursor)
    const page = await apiDropboxFileListPage(userId, parentId, 500, cursor)
    if (page.items.length) yield page.items.map(item => ({ ...mapDropboxFileToAliModel(item, driveId, parentId), user_id: userId } as IAliGetFileModel))
    cursor = page.hasMore ? page.cursor : ''
  } while (cursor && !shouldStop?.())
}
