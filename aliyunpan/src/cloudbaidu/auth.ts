import type { ITokenInfo } from '../user/userstore'
import { getProviderTokenForUser } from '../drive/account'
import { humanSize } from '../utils/format'
import { baiduFetch } from './request'

export { buildBaiduAuthUrl, exchangeBaiduCodeForToken, refreshBaiduAccessToken } from '../utils/baidu'

export const getBaiduToken = (userId: string) => getProviderTokenForUser(userId, 'baidu')

/** Refreshes account metadata without coupling the provider API to UserDAL. */
export const refreshBaiduUserInfo = async (token: ITokenInfo): Promise<boolean> => {
  if (!token.access_token) return false
  try {
    const userParams = new URLSearchParams({ method: 'uinfo', access_token: token.access_token, vip_version: 'v2' })
    const quotaParams = new URLSearchParams({ access_token: token.access_token, checkfree: '1', checkexpire: '1' })
    const [userResult, quotaResult] = await Promise.allSettled([
      baiduFetch(`https://pan.baidu.com/rest/2.0/xpan/nas?${userParams.toString()}`, { headers: { 'User-Agent': 'pan.baidu.com' } }),
      baiduFetch(`https://pan.baidu.com/api/quota?${quotaParams.toString()}`, { headers: { 'User-Agent': 'pan.baidu.com' } })
    ])
    if (userResult.status !== 'fulfilled') return false
    const userResp = userResult.value
    if (!userResp.ok) return false
    const user = await userResp.json()
    if (user?.errno !== 0) return false
    token.user_name = user.netdisk_name || user.baidu_name || token.user_name
    token.nick_name = user.netdisk_name || user.baidu_name || token.nick_name
    token.avatar = user.avatar_url || token.avatar
    if (user.vip_type === 2) token.vipname = 'SVIP'
    if (user.vip_type === 1) token.vipname = 'VIP'
    if (!token.user_id && user.uk) token.user_id = `baidu_${user.uk}`

    if (quotaResult.status !== 'fulfilled' || !quotaResult.value.ok) return true
    const quotaResp = quotaResult.value
    const quota = await quotaResp.json()
    if (quota?.errno !== 0) return true
    if (typeof quota.total === 'number') token.total_size = quota.total
    if (typeof quota.used === 'number') token.used_size = quota.used
    if (typeof quota.free === 'number') token.free_size = quota.free
    if (typeof quota.expire === 'boolean') token.space_expire = quota.expire
    if (typeof quota.total === 'number' && typeof quota.used === 'number') token.spaceinfo = humanSize(quota.used) + ' / ' + humanSize(quota.total)
    return true
  } catch {
    return false
  }
}
