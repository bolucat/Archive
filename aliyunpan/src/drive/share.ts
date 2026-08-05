import AliShare from '../aliapi/share'
import UserDAL from '../user/userdal'
import { resolveDriveProvider } from '../utils/driveProvider'
import { createCloud123PaidShare, createCloud123Share } from '../cloud123/adapter'
import { createPikPakShare } from '../pikpak/adapter'
import { createGuangyaShare } from '../guangya/adapter'
import { createDropboxShare } from '../dropbox/adapter'
import { createOneDriveShare } from '../onedrive/adapter'
import { createBoxShare } from '../box/adapter'
import { createGoogleShare } from '../google/adapter'
import { createQuarkShare } from '../quark/adapter'

/** Provider-neutral share creation. Provider APIs remain in their own directories. */
export default class DriveShare {
  static async ApiCreatCloud123PaidShare(userId: string, driveId: string, shareName: string, fileIds: string[], payAmount: number, resourceDesc = '', isReward = 0) {
    const route = resolveDriveProvider(userId, driveId, UserDAL.GetUserToken(userId)?.tokenfrom)
    if (!route.isValid || route.provider !== 'cloud123') return route.error || '当前网盘不支持付费分享'
    return createCloud123PaidShare(userId, driveId, fileIds, shareName, payAmount, resourceDesc, isReward)
  }

  static async ApiCreatShare(userId: string, driveId: string, expiration: string, sharePwd: string, shareName: string, fileIds: string[], isFolder = false) {
    if (!userId || !driveId || fileIds.length === 0) return '创建分享链接失败数据错误'
    const route = resolveDriveProvider(userId, driveId, UserDAL.GetUserToken(userId)?.tokenfrom)
    if (!route.isValid) return route.error

    switch (route.provider) {
      case 'cloud123': return createCloud123Share(userId, driveId, fileIds, shareName, expiration, sharePwd)
      case 'pikpak': return createPikPakShare(userId, driveId, fileIds, shareName, expiration, sharePwd)
      case 'guangya': return createGuangyaShare(userId, expiration, sharePwd, shareName, fileIds)
      case 'dropbox': return createDropboxShare(userId, driveId, fileIds, expiration, sharePwd, shareName)
      case 'onedrive': return createOneDriveShare(userId, driveId, fileIds, shareName, expiration, sharePwd)
      case 'box': return createBoxShare(userId, driveId, fileIds, shareName, isFolder, expiration, sharePwd)
      case 'google': return createGoogleShare(userId, driveId, fileIds, shareName, expiration, sharePwd)
      case 'quark': return createQuarkShare(userId, expiration, sharePwd, shareName, fileIds)
      case 'aliyun':
        return AliShare.ApiCreatShare(userId, driveId, expiration, sharePwd, shareName, fileIds, isFolder)
      default:
        return '当前网盘不支持创建分享链接'
    }
  }
}
