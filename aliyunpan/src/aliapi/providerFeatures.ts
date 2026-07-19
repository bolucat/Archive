import { getProviderCapabilities } from '../services/agent/providerCapabilities'
import type { DriveOperation } from '../services/agent/providerCapabilityTypes'
import { isAliyunUser, isBaiduUser, isBoxUser, isCloud123User, isCloud139User, isCloud189User, isDrive115User, isDropboxUser, isGuangyaUser, isOneDriveUser, isPikPakUser, isQuarkUser } from './utils'

function providerPlatform(userId: string, driveId: string): string {
  const drive = String(driveId || '').toLowerCase()
  if (isCloud123User(userId) || drive === 'cloud123' || drive === 'cloud_root') return 'cloud123'
  if (isDrive115User(userId) || drive === 'drive115' || drive === 'drive115_root') return '115'
  if (isCloud139User(userId) || drive === 'cloud139') return 'cloud139'
  if (isCloud189User(userId) || drive === 'cloud189') return 'cloud189'
  if (isBaiduUser(userId) || drive === 'baidu') return 'baidu'
  if (isGuangyaUser(userId) || drive === 'guangya' || drive === 'guangya_root') return 'guangya'
  if (isPikPakUser(userId) || drive === 'pikpak' || drive === 'pikpak_root') return 'pikpak'
  if (isQuarkUser(userId) || drive === 'quark' || drive === 'quark_root') return 'quark'
  if (isDropboxUser(userId) || drive === 'dropbox') return 'dropbox'
  if (isOneDriveUser(userId) || drive === 'onedrive') return 'onedrive'
  if (isBoxUser(userId) || drive === 'box') return 'box'
  if (isAliyunUser(userId)) return 'aliyun'
  return drive
}

export function supportsProviderOperation(userId: string, driveId: string, operation: DriveOperation): boolean {
  return Boolean(getProviderCapabilities(providerPlatform(userId, driveId)).operations[operation])
}

export const supportsCreateTextFile = (userId: string, driveId: string) => supportsProviderOperation(userId, driveId, 'upload.memory')
export const supportsLocalUpload = (userId: string, driveId: string) => supportsProviderOperation(userId, driveId, 'upload.local')
export const supportsEncryptedFileOperations = (userId: string) => supportsProviderOperation(userId, '', 'upload.encrypted')
export const supportsCreateShare = (userId: string, driveId: string) => supportsProviderOperation(userId, driveId, 'share.create')
export const supportsCopy = (userId: string, driveId: string) => supportsProviderOperation(userId, driveId, 'files.copy')
export const supportsTrashRestore = (userId: string, driveId: string) => supportsProviderOperation(userId, driveId, 'trash.restore')
export const supportsTrashPermanentDelete = (userId: string, driveId: string) => supportsProviderOperation(userId, driveId, 'trash.delete')
