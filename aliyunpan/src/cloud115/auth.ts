import type { ITokenInfo } from '../user/userstore'
import { getProviderTokenForUser } from '../drive/account'
import { humanSize } from '../utils/format'

export { build115UserId, refresh115AccessToken } from '../utils/drive115'

export const getDrive115Token = (userId: string) => getProviderTokenForUser(userId, '115')

/** Refreshes account metadata without coupling the provider API to UserDAL. */
export const refreshDrive115UserInfo = async (token: ITokenInfo): Promise<boolean> => {
  if (!token.access_token) return false
  try {
    const resp = await fetch('https://proapi.115.com/open/user/info', { headers: { Authorization: `Bearer ${token.access_token}` } })
    if (!resp.ok) return false
    const data = await resp.json()
    if (data?.code !== 0 || !data?.data) return false
    const info = data.data
    token.user_name = info.user_name || token.user_name
    token.nick_name = info.user_name || token.nick_name
    token.avatar = info.user_face_m || info.user_face_l || info.user_face_s || token.avatar
    const space = info.rt_space_info || {}
    const totalSize = space?.all_total?.size
    const usedSize = space?.all_use?.size
    if (typeof usedSize === 'number') token.used_size = usedSize
    if (typeof totalSize === 'number') token.total_size = totalSize
    if (typeof usedSize === 'number' && typeof totalSize === 'number') token.spaceinfo = humanSize(usedSize) + ' / ' + humanSize(totalSize)
    else if (space?.all_use?.size_format && space?.all_total?.size_format) token.spaceinfo = `${space.all_use.size_format} / ${space.all_total.size_format}`
    if (info?.vip_info?.level_name) token.vipname = info.vip_info.level_name
    if (info?.vip_info?.expire) token.vipexpire = String(info.vip_info.expire)
    return true
  } catch {
    return false
  }
}
