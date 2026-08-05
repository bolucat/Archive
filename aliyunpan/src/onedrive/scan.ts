import type { IAliGetFileModel } from '../aliapi/alimodels'
import { apiOneDriveFileListPage, mapOneDriveItemToAliModel } from './dirfilelist'

export async function* iterateOneDriveFolderPages(folder: IAliGetFileModel, userId: string, driveId: string, shouldStop?: () => boolean): AsyncGenerator<IAliGetFileModel[]> {
  const parentId = folder.file_id && !folder.file_id.includes('root') ? folder.file_id : 'onedrive_root'
  let nextLink = ''
  const seenLinks = new Set<string>()
  do {
    if (seenLinks.has(nextLink)) return
    seenLinks.add(nextLink)
    const page = await apiOneDriveFileListPage(userId, parentId, nextLink)
    if (page.items.length) yield page.items.map(item => ({ ...mapOneDriveItemToAliModel(item, driveId, parentId), user_id: userId } as IAliGetFileModel))
    nextLink = page.nextLink
  } while (nextLink && !shouldStop?.())
}
