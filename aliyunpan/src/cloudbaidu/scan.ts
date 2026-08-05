import type { IAliGetFileModel } from '../aliapi/alimodels'
import { apiBaiduFileList, mapBaiduFileToAliModel } from './dirfilelist'

export async function* iterateBaiduFolderPages(folder: IAliGetFileModel, userId: string, driveId: string, shouldStop?: () => boolean): AsyncGenerator<IAliGetFileModel[]> {
  const limit = 1000
  const path = (folder as any).path || folder.file_id || '/'
  for (let start = 0; !shouldStop?.(); start += limit) {
    const page = await apiBaiduFileList(userId, path, 'name', start, limit)
    if (page.length) yield page.map(item => ({ ...mapBaiduFileToAliModel(item, driveId, folder.file_id || ''), user_id: userId } as IAliGetFileModel))
    if (page.length < limit) return
  }
}
