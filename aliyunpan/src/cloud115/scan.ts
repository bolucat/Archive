import type { IAliGetFileModel } from '../aliapi/alimodels'
import { apiDrive115FileList, mapDrive115FileToAliModel } from './dirfilelist'

export async function* iterateDrive115FolderPages(folder: IAliGetFileModel, userId: string, driveId: string, silent = false, shouldStop?: () => boolean): AsyncGenerator<IAliGetFileModel[]> {
  const limit = 200
  for (let offset = 0; !shouldStop?.(); offset += limit) {
    const page = await apiDrive115FileList(userId, folder.file_id || '0', limit, offset, true, { silent })
    if (page.length) yield page.map(item => ({ ...mapDrive115FileToAliModel(item, driveId), user_id: userId } as IAliGetFileModel))
    if (page.length < limit) return
  }
}
