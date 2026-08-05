import type { IAliGetFileModel } from '../aliapi/alimodels'
import { listBoxSpecialItems } from '../box/adapter'
import UserDAL from '../user/userdal'
import { resolveDriveProvider } from '../utils/driveProvider'

export type ProviderSpecialListResult = {
  items: IAliGetFileModel[]
  total: number
}

/** Thin registry for provider-owned virtual lists such as recent, favorites, and trash. */
export const listProviderSpecialItems = async (userId: string, driveId: string, dirId: string, maxItems: number): Promise<ProviderSpecialListResult | undefined> => {
  const route = resolveDriveProvider(userId, driveId, UserDAL.GetUserToken(userId)?.tokenfrom)
  if (!route.isValid) return undefined
  switch (route.provider) {
    case 'box': return await listBoxSpecialItems(userId, dirId, maxItems)
    default: return undefined
  }
}
