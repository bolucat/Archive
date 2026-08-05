import type { IAliGetFileModel } from '../aliapi/alimodels'
import { apiGuangyaFileListPage, mapGuangyaFileToAliModel } from './dirfilelist'

export async function* iterateGuangyaFolderPages(folder: IAliGetFileModel, userId: string, driveId: string, shouldStop?: () => boolean): AsyncGenerator<IAliGetFileModel[]> {
  const parentId = folder.file_id || 'guangya_root'
  for (let pageNumber = 0; !shouldStop?.(); pageNumber++) {
    const page = await apiGuangyaFileListPage(userId, parentId, pageNumber)
    if (page.items.length) yield page.items.map(item => ({ ...mapGuangyaFileToAliModel(item, driveId, parentId), user_id: userId } as IAliGetFileModel))
    if (!page.hasMore) return
  }
}
