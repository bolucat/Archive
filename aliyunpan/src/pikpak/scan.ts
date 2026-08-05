import type { IAliGetFileModel } from '../aliapi/alimodels'
import { apiPikPakFileList, mapPikPakFileToAliModel } from './dirfilelist'

export async function* iteratePikPakFolderPages(folder: IAliGetFileModel, userId: string, driveId: string, shouldStop?: () => boolean): AsyncGenerator<IAliGetFileModel[]> {
  const parentId = folder.file_id && !folder.file_id.includes('root') ? folder.file_id : 'pikpak_root'
  let pageToken = ''
  const seenTokens = new Set<string>()
  do {
    if (seenTokens.has(pageToken)) return
    seenTokens.add(pageToken)
    const page = await apiPikPakFileList(userId, parentId, 500, pageToken)
    if (page.items.length) yield page.items.map(item => ({ ...mapPikPakFileToAliModel(item, driveId, parentId), user_id: userId } as IAliGetFileModel))
    pageToken = page.nextPageToken
  } while (pageToken && !shouldStop?.())
}
