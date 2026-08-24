import type { IAliGetFileModel } from '../aliapi/alimodels'
import { resolveDriveProvider, type DriveProvider } from './driveProvider'
import { listProviderItems } from '../drive/providerList'
import { iterateProviderPages } from '../drive/providerPagination'
import { libraryScanRateLimitScope, runRateLimitedScanRequest } from './libraryScanRateLimiter'

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
  if (!route.isValid || !isScanProvider(route.provider)) return
  const scope = libraryScanRateLimitScope(userId, driveId)
  yield* iterateProviderPages(cursor => runRateLimitedScanRequest(scope, async () => {
    const page = await listProviderItems(route.provider, userId, driveId, folder.file_id, true, cursor, { skipThumbnailHydration: true })
    if (!page) throw new Error(`Unsupported provider pagination route: ${route.provider}`)
    return page
  }), options.shouldStop)
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
