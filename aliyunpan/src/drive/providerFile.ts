import type { IAliGetFileModel } from '../aliapi/alimodels'
import type { IVideoPreviewUrl } from '../aliapi/models'
import { getCloud123FileInfo, getCloud123VideoPreview } from '../cloud123/adapter'
import { getDrive115FileInfo, getDrive115VideoPreview, updateDrive115VideoHistory } from '../cloud115/adapter'
import { getBaiduFileInfo } from '../cloudbaidu/adapter'
import { getPikPakFileInfo } from '../pikpak/adapter'
import { getQuarkFileInfo, getQuarkVideoPreview } from '../quark/adapter'
import { getCloud139FileInfo } from '../cloud139/adapter'
import { getCloud189FileInfo } from '../cloud189/adapter'
import { getGuangyaFileInfo } from '../guangya/adapter'
import { getDropboxFileInfo } from '../dropbox/adapter'
import { getOneDriveFileInfo } from '../onedrive/adapter'
import { getBoxFileInfo } from '../box/adapter'
import { getGoogleFileInfo } from '../google/adapter'
import type { DriveProvider } from '../utils/driveProvider'

/** Thin registry for provider-owned file adapters. */
export const getProviderFileInfo = async (provider: DriveProvider, userId: string, driveId: string, fileId: string): Promise<IAliGetFileModel | undefined> => {
  switch (provider) {
    case 'cloud123': return await getCloud123FileInfo(userId, fileId) as IAliGetFileModel | undefined
    case '115': return await getDrive115FileInfo(userId, driveId, fileId) as IAliGetFileModel | undefined
    case 'baidu': return await getBaiduFileInfo(userId, driveId, fileId) as IAliGetFileModel | undefined
    case 'pikpak': return await getPikPakFileInfo(userId, driveId, fileId) as IAliGetFileModel | undefined
    case 'quark': return await getQuarkFileInfo(userId, driveId, fileId) as IAliGetFileModel | undefined
    case '139': return await getCloud139FileInfo(userId, driveId, fileId) as IAliGetFileModel | undefined
    case '189': return await getCloud189FileInfo(userId, driveId, fileId) as IAliGetFileModel | undefined
    case 'guangya': return await getGuangyaFileInfo(userId, driveId, fileId) as IAliGetFileModel | undefined
    case 'dropbox': return await getDropboxFileInfo(userId, driveId, fileId) as IAliGetFileModel | undefined
    case 'onedrive': return await getOneDriveFileInfo(userId, driveId, fileId) as IAliGetFileModel | undefined
    case 'box': return await getBoxFileInfo(userId, driveId, fileId) as IAliGetFileModel | undefined
    case 'google': return await getGoogleFileInfo(userId, driveId, fileId) as IAliGetFileModel | undefined
    default: return undefined
  }
}

export const getProviderVideoPreview = (provider: DriveProvider, userId: string, driveId: string, fileId: string): Promise<IVideoPreviewUrl | string | undefined> => {
  switch (provider) {
    case 'cloud123': return getCloud123VideoPreview(userId, driveId, fileId)
    case '115': return getDrive115VideoPreview(userId, driveId, fileId)
    case 'quark': return getQuarkVideoPreview(userId, fileId)
    default: return Promise.resolve(undefined)
  }
}

export const updateProviderVideoHistory = async (provider: DriveProvider, userId: string, fileId: string, playCursor: number): Promise<void> => {
  if (provider === '115') await updateDrive115VideoHistory(userId, fileId, playCursor)
}
