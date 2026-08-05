import type { IAliGetFileModel } from '../aliapi/alimodels'
import UserDAL from '../user/userdal'
import type { ITokenInfo } from '../user/userstore'
import { isProviderTokenForUser, isTokenCompatibleWithDrive } from '../utils/driveProvider'

export { isProviderTokenForUser }

/** Never borrow another account's token when a provider API is called by user ID. */
export const getProviderTokenForUser = async (userId: string, provider: ITokenInfo['tokenfrom']): Promise<ITokenInfo | undefined> => {
  const cached = UserDAL.GetUserToken(userId)
  if (isProviderTokenForUser(cached, userId, provider)) return cached

  const stored = await UserDAL.GetUserTokenFromDB(userId)
  return isProviderTokenForUser(stored, userId, provider) ? stored : undefined
}

export const resolveDriveFileToken = async (file: Pick<IAliGetFileModel, 'drive_id'> & { user_id?: string }, preferredUserId = ''): Promise<ITokenInfo | undefined> => {
  const candidateUserIds = [file.user_id, preferredUserId].filter((userId, index, values): userId is string => !!userId && values.indexOf(userId) === index)
  for (const userId of candidateUserIds) {
    const token = await UserDAL.GetUserTokenFromDB(userId)
    if (token?.access_token && isTokenCompatibleWithDrive(token, file.drive_id || '')) return token
  }
  return undefined
}
