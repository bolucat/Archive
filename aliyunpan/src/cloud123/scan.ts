import type { IAliGetFileModel } from '../aliapi/alimodels'
import { apiCloud123FileListPage, mapCloud123FileToAliModel } from './dirfilelist'

export async function* iterateCloud123FolderPages(folder: IAliGetFileModel, userId: string, driveId: string, shouldStop?: () => boolean): AsyncGenerator<IAliGetFileModel[]> {
  let lastFileId: string | number = ''
  while (!shouldStop?.()) {
    const page = await apiCloud123FileListPage(userId, folder.file_id || '0', 100, false, '', 0, lastFileId)
    if (page.items.length) yield page.items.map(item => ({ ...mapCloud123FileToAliModel(item), drive_id: driveId, user_id: userId } as IAliGetFileModel))
    if (page.lastFileId < 0 || !page.items.length || String(page.lastFileId) === String(lastFileId)) return
    lastFileId = page.lastFileId
  }
}
