import type { IAliGetFileModel } from '../aliapi/alimodels'
import { apiQuarkFileList, mapQuarkFileToAliModel } from './dirfilelist'

export async function* iterateQuarkFolderPages(folder: IAliGetFileModel, userId: string, driveId: string, shouldStop?: () => boolean): AsyncGenerator<IAliGetFileModel[]> {
  const parentId = folder.file_id || '0'
  for (let pageNumber = 1, total = Infinity; pageNumber <= Math.ceil(total / 100) && !shouldStop?.(); pageNumber++) {
    const page = await apiQuarkFileList(userId, parentId, 100, pageNumber)
    total = page.total
    if (page.items.length) yield page.items.map(item => ({ ...mapQuarkFileToAliModel(item, driveId, parentId), user_id: userId } as IAliGetFileModel))
    if (!page.items.length) return
  }
}
