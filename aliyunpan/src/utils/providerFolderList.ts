import type { IAliGetFileModel } from '../aliapi/alimodels'
import { resolveDriveProvider, type DriveProvider } from './driveProvider'
import { iterateCloud123FolderPages } from '../cloud123/scan'
import { iterateDrive115FolderPages } from '../cloud115/scan'
import { iterateBaiduFolderPages } from '../cloudbaidu/scan'
import { iteratePikPakFolderPages } from '../pikpak/scan'
import { iterateQuarkFolderPages } from '../quark/scan'
import { iterateCloud139FolderPages } from '../cloud139/scan'
import { iterateCloud189FolderPages } from '../cloud189/scan'
import { iterateDropboxFolderPages } from '../dropbox/scan'
import { iterateOneDriveFolderPages } from '../onedrive/scan'
import { iterateBoxFolderPages } from '../box/scan'
import { iterateGuangyaFolderPages } from '../guangya/scan'
import { iterateGoogleFolderPages } from '../google/scan'

export type ProviderFolderListOptions = {
  folder: IAliGetFileModel
  userId: string
  driveId: string
  silent?: boolean
  shouldStop?: () => boolean
}

const scanProviders: DriveProvider[] = ['cloud123', '115', 'baidu', 'pikpak', 'quark', '139', '189', 'dropbox', 'onedrive', 'box', 'guangya', 'google']

const isScanProvider = (provider: DriveProvider) => scanProviders.includes(provider)

export const isThirdPartyProviderFolder = (userId: string, driveId: string): boolean => {
  const route = resolveDriveProvider(userId, driveId)
  // A mismatched third-party account and drive must not fall through to Aliyun.
  return isScanProvider(route.provider) || isScanProvider(route.accountProvider) || isScanProvider(route.driveProvider)
}

export async function* iterateProviderFolderPages(options: ProviderFolderListOptions): AsyncGenerator<IAliGetFileModel[]> {
  const { folder, userId, driveId } = options
  const route = resolveDriveProvider(userId, driveId)
  if (!route.isValid) return

  if (route.provider === 'cloud123') {
    yield* iterateCloud123FolderPages(folder, userId, driveId, options.shouldStop)
    return
  }
  if (route.provider === '115') {
    yield* iterateDrive115FolderPages(folder, userId, driveId, !!options.silent, options.shouldStop)
    return
  }
  if (route.provider === 'baidu') {
    yield* iterateBaiduFolderPages(folder, userId, driveId, options.shouldStop)
    return
  }
  if (route.provider === 'pikpak') {
    yield* iteratePikPakFolderPages(folder, userId, driveId, options.shouldStop)
    return
  }
  if (route.provider === 'quark') {
    yield* iterateQuarkFolderPages(folder, userId, driveId, options.shouldStop)
    return
  }
  if (route.provider === '139') {
    yield* iterateCloud139FolderPages(folder, userId, driveId, options.shouldStop)
    return
  }
  if (route.provider === '189') {
    yield* iterateCloud189FolderPages(folder, userId, driveId, options.shouldStop)
    return
  }

  if (route.provider === 'dropbox') {
    yield* iterateDropboxFolderPages(folder, userId, driveId, options.shouldStop)
    return
  }
  if (route.provider === 'onedrive') {
    yield* iterateOneDriveFolderPages(folder, userId, driveId, options.shouldStop)
    return
  }
  if (route.provider === 'box') {
    yield* iterateBoxFolderPages(folder, userId, driveId, options.shouldStop)
    return
  }
  if (route.provider === 'guangya') {
    yield* iterateGuangyaFolderPages(folder, userId, driveId, options.shouldStop)
    return
  }
  if (route.provider === 'google') {
    yield* iterateGoogleFolderPages(folder, userId, driveId, options.shouldStop)
  }
}

/**
 * Lists every page of a third-party provider directory and maps it to the
 * common file model. Returns null for Aliyun drives, which keep their own
 * marker-based listing path.
 */
export async function listProviderFolderItems(options: ProviderFolderListOptions): Promise<IAliGetFileModel[] | null> {
  if (!isThirdPartyProviderFolder(options.userId, options.driveId)) return null
  const items: IAliGetFileModel[] = []
  for await (const page of iterateProviderFolderPages(options)) items.push(...page)
  return items
}
