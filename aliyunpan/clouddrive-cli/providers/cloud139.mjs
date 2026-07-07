import { createHash } from 'node:crypto'

const ROUTE_POLICY_URL = 'https://user-njs.yun.139.com/user/route/qryRoutePolicy'
const DEFAULT_HOST = 'https://ose.caiyun.feixin.10086.cn'
const USER_AGENT = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
const DEVICE_INFO = '||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||'
const CLIENT_INFO = '||9|7.14.0|chrome|120.0.0.0|||windows 10||zh-CN|||dW5kZWZpbmVk||'

const hostCache = new Map()

function md5(value) {
  return createHash('md5').update(value).digest('hex')
}

function randomString(length = 16) {
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
  let value = ''
  for (let i = 0; i < length; i++) value += chars[Math.floor(Math.random() * chars.length)]
  return value
}

function calcSign(body, ts, rand) {
  const sorted = encodeURIComponent(body).split('').sort().join('')
  const bodyBase64 = Buffer.from(sorted, 'utf8').toString('base64')
  return md5(`${md5(bodyBase64)}${md5(`${ts}:${rand}`)}`).toUpperCase()
}

function authorization(token) {
  return String(token.access_token || token.authorization || '').replace(/^Basic\s+/i, '').trim()
}

function parseAccount(token) {
  if (token.user_name && !String(token.user_name).startsWith('139云盘')) return token.user_name
  try {
    const decoded = Buffer.from(authorization(token), 'base64').toString('utf8')
    return decoded.split(':')[1] || token.user_id || ''
  } catch {
    return token.user_id || ''
  }
}

function headers(token, body, route = false, svcType = '1') {
  const ts = String(Date.now())
  const rand = randomString()
  const base = {
    Accept: 'application/json, text/plain, */*',
    'Content-Type': 'application/json;charset=UTF-8',
    'Accept-Language': 'zh-CN,zh;q=0.9,en;q=0.8',
    'User-Agent': USER_AGENT,
    Origin: 'https://yun.139.com',
    Referer: 'https://yun.139.com/w/',
    'x-DeviceInfo': DEVICE_INFO,
    'x-huawei-channelSrc': '10000034',
    'x-inner-ntwk': '2',
    'x-m4c-caller': 'PC',
    'x-m4c-src': '10002',
    'Inner-Hcy-Router-Https': '1',
    'x-SvcType': svcType,
    Authorization: `Basic ${authorization(token)}`,
    'mcloud-sign': `${ts},${rand},${calcSign(body, ts, rand)}`,
  }
  if (route) return base
  return {
    ...base,
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
    'x-yun-svc-type': svcType,
  }
}

async function postJson(url, token, body, route = false) {
  const raw = JSON.stringify(body || {})
  const resp = await fetch(url, { method: 'POST', headers: headers(token, raw, route), body: raw })
  const data = await resp.json().catch(() => undefined)
  if (!resp.ok || data?.success === false || ['9000', '9008', '9100', '100002'].includes(String(data?.code || ''))) {
    const err = new Error(data?.message || data?.msg || `139 API ${resp.status}`)
    err.code = data?.code || 'ERR_139_API'
    err.status = resp.status
    throw err
  }
  return data
}

async function getHost(token) {
  const accountId = token.user_id || authorization(token).slice(0, 12)
  if (hostCache.has(accountId)) return hostCache.get(accountId)
  const account = parseAccount(token)
  const data = await postJson(ROUTE_POLICY_URL, token, {
    userInfo: { userType: 1, accountType: 1, accountName: account },
    modAddrType: 1,
  }, true)
  const routes = data?.data?.routePolicyList || data?.routePolicyList || []
  const personal = Array.isArray(routes) ? routes.find((item) => item?.modName === 'personal') || routes[0] : null
  const host = personal?.httpsUrl || personal?.httpUrl || DEFAULT_HOST
  hostCache.set(accountId, host)
  return host
}

async function cloud139Request(token, endpoint, body) {
  const host = await getHost(token)
  return postJson(`${host}/orchestration${endpoint}`, token, body)
}

function parentId(id) {
  return !id || id === 'cloud139_root' || id === '0' ? '/' : String(id)
}

function listFromResponse(data) {
  const raw = data?.data || data || {}
  const list = raw.list || raw.content || raw.files || raw.fileList || []
  return Array.isArray(list) ? list : []
}

function mapItem(item, accountId, parent = 'cloud139_root') {
  const isFolder = item.type === 'folder'
  return {
    provider: '139',
    accountId,
    driveId: 'cloud139',
    fileId: String(item.fileId || item.catalogId || ''),
    parentFileId: String(item.parentFileId || item.parentCatalogId || parent),
    name: item.name || item.catalogName || '',
    type: isFolder ? 'folder' : 'file',
    size: Number(item.size || 0),
    updatedAt: item.updated_at || item.updatedAt || item.updateTime,
    createdAt: item.created_at || item.createTime,
  }
}

export async function cloud139RefreshToken(token) {
  return token
}

export async function cloud139ListDir(token, currentParentId = 'cloud139_root', { limit = 100 } = {}) {
  const data = await cloud139Request(token, '/file/list', {
    imageThumbnailStyleList: ['Small', 'Large'],
    orderBy: 'updated_at',
    orderDirection: 'DESC',
    pageInfo: { pageCursor: '', pageSize: limit },
    parentFileId: parentId(currentParentId),
  })
  return listFromResponse(data).map((item) => mapItem(item, token.user_id, currentParentId))
}

export async function* cloud139Walk(token, currentParentId = 'cloud139_root', maxDepth = 10) {
  const queue = [{ parentFileId: currentParentId, depth: 0 }]
  while (queue.length) {
    const current = queue.shift()
    const items = await cloud139ListDir(token, current.parentFileId)
    for (const item of items) {
      yield item
      if (item.type === 'folder' && current.depth < maxDepth) queue.push({ parentFileId: item.fileId, depth: current.depth + 1 })
    }
  }
}

export async function cloud139GetFile(token, fileId) {
  if (fileId === 'cloud139_root' || fileId === '/' || fileId === '0') return { provider: '139', accountId: token.user_id, driveId: 'cloud139', fileId: 'cloud139_root', parentFileId: '', name: '网盘文件', type: 'folder' }
  const rootItems = await cloud139ListDir(token, 'cloud139_root')
  const item = rootItems.find((entry) => entry.fileId === String(fileId))
  if (item) return item
  const err = new Error(`File not found: ${fileId}`)
  err.code = 'ERR_139_NOT_FOUND'
  throw err
}

export async function cloud139RenameBatch(token, renames) {
  const results = []
  for (const { fileId, newName } of renames) {
    try {
      await cloud139Request(token, '/file/update', { fileId, fileName: newName })
      results.push({ fileId, status: 'success', newName })
    } catch (e) {
      results.push({ fileId, status: 'error', code: e.code || 'ERR', message: e.message })
    }
  }
  return results
}

export async function cloud139MoveBatch(token, moves) {
  const groups = new Map()
  for (const move of moves) {
    const key = parentId(move.toParentId)
    if (!groups.has(key)) groups.set(key, [])
    groups.get(key).push(move)
  }
  const results = []
  for (const [target, group] of groups) {
    try {
      await cloud139Request(token, '/file/batchMove', { fileIds: group.map((item) => item.fileId), parentFileId: target })
      results.push(...group.map(({ fileId }) => ({ fileId, status: 'success' })))
    } catch (e) {
      results.push(...group.map(({ fileId }) => ({ fileId, status: 'error', code: e.code || 'ERR', message: e.message })))
    }
  }
  return results
}

export async function cloud139Mkdir(token, currentParentId = 'cloud139_root', name) {
  const data = await cloud139Request(token, '/file/create', { parentFileId: parentId(currentParentId), fileName: name, type: 'folder' })
  const fileId = data?.data?.fileId || data?.data?.catalogId || data?.fileId || data?.catalogId || ''
  return { provider: '139', accountId: token.user_id, driveId: 'cloud139', fileId: String(fileId), parentFileId: String(currentParentId), name, type: 'folder' }
}

export async function cloud139Trash(token, fileIds) {
  if (!fileIds.length) return []
  await cloud139Request(token, '/recyclebin/batchTrash', { fileIds })
  return fileIds.map((fileId) => ({ fileId, status: 'success' }))
}
