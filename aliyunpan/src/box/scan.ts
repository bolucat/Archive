import type { IAliGetFileModel } from '../aliapi/alimodels'
import { apiBoxFileListPage, mapBoxItemToAliModel } from './dirfilelist'

export async function* iterateBoxFolderPages(folder: IAliGetFileModel, userId: string, driveId: string, shouldStop?: () => boolean): AsyncGenerator<IAliGetFileModel[]> {
  const parentId = folder.file_id && !folder.file_id.includes('root') ? folder.file_id : 'box_root'
  let offset = 0
  const seenOffsets = new Set<number>()
  while (!shouldStop?.()) {
    if (seenOffsets.has(offset)) return
    seenOffsets.add(offset)
    const page = await apiBoxFileListPage(userId, parentId, 500, offset)
    if (page.items.length) yield page.items.map(item => ({ ...mapBoxItemToAliModel(item, driveId, parentId), user_id: userId } as IAliGetFileModel))
    if (page.nextOffset === null) return
    offset = page.nextOffset
  }
}
