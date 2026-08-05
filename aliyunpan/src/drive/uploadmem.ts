import UserDAL from '../user/userdal'
import { resolveDriveProvider } from '../utils/driveProvider'
import AliUploadMem from '../aliapi/uploadmem'
import { uploadDropboxTextFile } from '../dropbox/adapter'
import { uploadOneDriveTextFile } from '../onedrive/adapter'
import { uploadGoogleTextFile } from '../google/adapter'
import { uploadBoxTextFile } from '../box/adapter'
import { uploadGuangyaTextFile } from '../guangya/adapter'

/** Provider-neutral entry point. Provider implementations stay in their own directories. */
export default class DriveUploadMem {
  static async UploadMem(user_id: string, drive_id: string, parent_file_id: string, creatFileName: string, context: string, encType: string = '') {
    const token = await UserDAL.GetUserTokenFromDB(user_id)
    if (!token || !token.access_token) return '账号失效，操作取消'

    const route = resolveDriveProvider(user_id, drive_id, token.tokenfrom)
    if (!route.isValid) return route.error

    switch (route.provider) {
      case 'dropbox': return uploadDropboxTextFile(token.access_token, parent_file_id, creatFileName, context, encType)
      case 'onedrive': return uploadOneDriveTextFile(user_id, parent_file_id, creatFileName, context, encType)
      case 'box': return uploadBoxTextFile(user_id, parent_file_id, creatFileName, context, encType)
      case 'google': return uploadGoogleTextFile(user_id, parent_file_id, creatFileName, context)
      case 'guangya': return uploadGuangyaTextFile(user_id, parent_file_id, creatFileName, context, encType)
      case 'aliyun':
        return AliUploadMem.UploadMem(user_id, drive_id, parent_file_id, creatFileName, context, encType)
      default:
        return '当前网盘暂不支持新建文本文件'
    }
  }
}
