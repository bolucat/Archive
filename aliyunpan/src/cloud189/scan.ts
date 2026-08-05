import type { IAliGetFileModel } from '../aliapi/alimodels'
import { apiCloud189FileList, mapCloud189FileToAliModel } from './dirfilelist'

export async function* iterateCloud189FolderPages(folder: IAliGetFileModel, userId: string, driveId: string, shouldStop?: () => boolean): AsyncGenerator<IAliGetFileModel[]> {
  const parentId = folder.file_id || 'cloud189_root'
  const limit = 1000
  for (let pageNum = 1; !shouldStop?.(); pageNum++) {
    const page = await apiCloud189FileList(userId, parentId, limit, pageNum)
    if (page.length) yield page.map(item => ({ ...mapCloud189FileToAliModel(item, driveId, parentId), user_id: userId } as IAliGetFileModel))
    if (page.length < limit) return
  }
}
