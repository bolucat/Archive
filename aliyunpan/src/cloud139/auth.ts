import { MD5 } from 'crypto-js'
import type { ITokenInfo } from '../user/userstore'

export const CLOUD139_USER_AGENT = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
export const CLOUD139_ROUTE_POLICY_URL = 'https://user-njs.yun.139.com/user/route/qryRoutePolicy'

const DEVICE_INFO = '||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||'
const CLIENT_INFO = '||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||dW5kZWZpbmVk||'

const hashString = (value: string): string => {
  let hash = 0
  for (let i = 0; i < value.length; i++) {
    hash = (hash << 5) - hash + value.charCodeAt(i)
    hash |= 0
  }
  return Math.abs(hash).toString(36)
}

export const parseCloud139Authorization = (authorization: string) => {
  const normalized = authorization.replace(/^Basic\s+/i, '').trim()
  const decoded = decodeURIComponent(Array.prototype.map.call(atob(normalized), (c: string) => `%${c.charCodeAt(0).toString(16).padStart(2, '0')}`).join(''))
  const parts = decoded.split(':')
  if (parts.length < 3) throw new Error('Authorization 令牌格式不完整')
  const account = parts[1]
  const tokenInfo = parts.slice(2).join(':')
  const tokenParts = tokenInfo.split('|')
  const expireTime = Number(tokenParts[3] || 0)
  return { authorization: normalized, account, tokenInfo, expireTime }
}

export const cloud139Sign = (body: string, ts: string, rand: string) => {
  const encoded = encodeURIComponent(body)
  const sorted = encoded.split('').sort().join('')
  const bodyBase64 = btoa(unescape(encodeURIComponent(sorted)))
  const hash1 = MD5(bodyBase64).toString()
  const hash2 = MD5(`${ts}:${rand}`).toString()
  return MD5(`${hash1}${hash2}`).toString().toUpperCase()
}

const randomString = (length = 16) => {
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
  let value = ''
  for (let i = 0; i < length; i++) value += chars[Math.floor(Math.random() * chars.length)]
  return value
}

export const cloud139Headers = (authorization: string, body: string, route = false, svcType = '1') => {
  const ts = String(Date.now())
  const rand = randomString()
  const sign = cloud139Sign(body, ts, rand)
  return {
    Accept: 'application/json, text/plain, */*',
    'Content-Type': 'application/json;charset=UTF-8',
    'Accept-Language': 'zh-CN,zh;q=0.9,en;q=0.8',
    'User-Agent': CLOUD139_USER_AGENT,
    Origin: 'https://yun.139.com',
    Referer: 'https://yun.139.com/w/',
    'x-DeviceInfo': DEVICE_INFO,
    'x-huawei-channelSrc': '10000034',
    'x-inner-ntwk': '2',
    'x-m4c-caller': 'PC',
    'x-m4c-src': '10002',
    'Inner-Hcy-Router-Https': '1',
    'x-SvcType': svcType,
    Authorization: `Basic ${authorization}`,
    'mcloud-sign': `${ts},${rand},${sign}`,
    ...(route ? {} : {
      Caller: 'web',
      'CMS-DEVICE': 'default',
      'mcloud-route': '001',
      'mcloud-channel': '1000101',
      'mcloud-client': '10701',
      'mcloud-version': '7.14.0',
      'x-yun-api-version': 'v1',
      'x-yun-app-channel': '10000034',
      'x-yun-channel-source': '10000034',
      'x-yun-client-info': CLIENT_INFO,
      'x-yun-module-type': '100',
      'x-yun-svc-type': svcType
    })
  }
}

export const normalizeCloud139Token = (authorizationInput: string): ITokenInfo => {
  const { authorization, account, expireTime } = parseCloud139Authorization(authorizationInput)
  const id = account || hashString(authorization)
  return {
    tokenfrom: '139',
    access_token: authorization,
    refresh_token: '',
    session_expires_in: 0,
    open_api_token_type: '',
    open_api_access_token: '',
    open_api_refresh_token: '',
    open_api_expires_in: 0,
    signature: '',
    device_id: '',
    expires_in: 0,
    token_type: 'Basic',
    user_id: `cloud139_${id}`,
    user_name: account || '139云盘',
    avatar: '',
    nick_name: account ? `139云盘 ${account}` : '139云盘',
    default_drive_id: 'cloud139',
    default_sbox_drive_id: '',
    resource_drive_id: '',
    backup_drive_id: '',
    sbox_drive_id: '',
    role: '',
    status: '',
    expire_time: expireTime ? new Date(expireTime).toISOString() : '',
    state: '',
    pin_setup: false,
    is_first_login: false,
    need_rp_verify: false,
    name: account || '139云盘',
    spu_id: '',
    is_expires: false,
    used_size: 0,
    total_size: 0,
    free_size: 0,
    space_expire: false,
    spaceinfo: '',
    vipname: '',
    vipIcon: '',
    vipexpire: '',
    pic_drive_id: '',
    signInfo: { signMon: -1, signDay: -1 }
  }
}
