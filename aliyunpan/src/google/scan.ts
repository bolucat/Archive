import type { IAliGetFileModel } from '../aliapi/alimodels'
import { apiGoogleFileList, apiGoogleSharedDriveFileList, getGoogleSharedDriveIdForFile, mapGoogleFileToAliModel } from './dirfilelist'

export async function* iterateGoogleFolderPages(folder: IAliGetFileModel, userId: string, driveId: string, shouldStop?: () => boolean): AsyncGenerator<IAliGetFileModel[]> {
  if (shouldStop?.()) return
  const parentId = folder.file_id && !folder.file_id.includes('root') ? folder.file_id : 'google_root'
  const sharedDriveId = parentId.startsWith('google_shared_drive:') ? parentId.slice('google_shared_drive:'.length) : getGoogleSharedDriveIdForFile(parentId)
  const items = sharedDriveId
    ? await apiGoogleSharedDriveFileList(userId, sharedDriveId, parentId.startsWith('google_shared_drive:') ? 'root' : parentId)
    : await apiGoogleFileList(userId, parentId)
  if (items.length) yield items.map(item => ({ ...mapGoogleFileToAliModel(item, driveId, parentId), user_id: userId } as IAliGetFileModel))
}
