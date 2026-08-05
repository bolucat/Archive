import { listBoxItems } from '../box/adapter'
import { listBaiduItems } from '../cloudbaidu/adapter'
import { listCloud123Items } from '../cloud123/adapter'
import { listCloud139Items } from '../cloud139/adapter'
import { listCloud189Items } from '../cloud189/adapter'
import { listDrive115Items } from '../cloud115/adapter'
import { listDropboxItems } from '../dropbox/adapter'
import { listGoogleItems } from '../google/adapter'
import { listGuangyaItems } from '../guangya/adapter'
import { listOneDriveItems } from '../onedrive/adapter'
import { listPikPakItems } from '../pikpak/adapter'
import { listQuarkItems } from '../quark/adapter'
import type { IAliGetFileModel } from '../aliapi/alimodels'
import type { DriveProvider } from '../utils/driveProvider'

export type ProviderListResult = {
  items: IAliGetFileModel[]
  total: number
  error?: string
}

/** Thin registry for provider-owned directory, search, and thumbnail adapters. */
export const listProviderItems = (provider: DriveProvider, userId: string, driveId: string, dirId: string, includeFiles: boolean): Promise<ProviderListResult> | undefined => {
  switch (provider) {
    case 'baidu': return listBaiduItems(userId, driveId, dirId, includeFiles)
    case 'cloud123': return listCloud123Items(userId, driveId, dirId, includeFiles)
    case '139': return listCloud139Items(userId, driveId, dirId, includeFiles)
    case '189': return listCloud189Items(userId, driveId, dirId, includeFiles)
    case 'guangya': return listGuangyaItems(userId, driveId, dirId, includeFiles)
    case '115': return listDrive115Items(userId, driveId, dirId, includeFiles)
    case 'pikpak': return listPikPakItems(userId, driveId, dirId, includeFiles)
    case 'quark': return listQuarkItems(userId, driveId, dirId, includeFiles)
    case 'box': return listBoxItems(userId, driveId, dirId, includeFiles)
    case 'dropbox': return listDropboxItems(userId, driveId, dirId, includeFiles)
    case 'onedrive': return listOneDriveItems(userId, driveId, dirId, includeFiles)
    case 'google': return listGoogleItems(userId, driveId, dirId, includeFiles)
    default: return undefined
  }
}
