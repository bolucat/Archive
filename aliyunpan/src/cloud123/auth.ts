import type { ITokenInfo } from '../user/userstore'
import { getProviderTokenForUser } from '../drive/account'
import { humanSize } from '../utils/format'

export { buildCloud123AuthUrl, exchangeCloud123CodeForToken, refreshCloud123AccessToken } from '../utils/cloud123'

export const getCloud123Token = (userId: string) => getProviderTokenForUser(userId, 'cloud123')

/** Refreshes account metadata without coupling the provider API to UserDAL. */
export const refreshCloud123UserInfo = async (token: ITokenInfo): Promise<boolean> => {
  if (!token.access_token) return false
  try {
    const resp = await fetch('https://open-api.123pan.com/api/v1/user/info', {
      headers: {
        'Content-Type': 'application/json',
        Platform: 'open_platform',
        Authorization: `Bearer ${token.access_token}`
      }
    })
    if (!resp.ok) return false
    const data = await resp.json()
    if (data?.code !== 0 || !data?.data) return false
    const info = data.data
    token.user_name = info.nickname || token.user_name
    token.nick_name = info.nickname || token.nick_name
    token.avatar = info.headImage || token.avatar
    if (typeof info.spaceUsed === 'number') token.used_size = info.spaceUsed
    if (typeof info.spacePermanent === 'number') token.total_size = info.spacePermanent
    if (typeof info.spaceUsed === 'number' && typeof info.spacePermanent === 'number') token.spaceinfo = humanSize(info.spaceUsed) + ' / ' + humanSize(info.spacePermanent)
    const vipCurrent = Array.isArray(info.vipInfo) ? info.vipInfo[0] : undefined
    if (vipCurrent?.vipLabel) token.vipname = vipCurrent.vipLabel
    if (vipCurrent?.endTime) token.vipexpire = vipCurrent.endTime
    if (info.vip) token.vipIcon = token.vipIcon || ''
    return true
  } catch {
    return false
  }
}
