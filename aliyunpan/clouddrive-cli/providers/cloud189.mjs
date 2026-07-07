import { createHmac } from 'node:crypto'

const AUTH_URL = 'https://open.e.189.cn'
const API_URL = 'https://api.cloud.189.cn'
const WEB_URL = 'https://cloud.189.cn'
const APP_ID = '8025431004'
const VERSION = '6.2'
const PC = 'TELEPC'
const CHANNEL_ID = 'web_cloud.189.cn'
const USER_AGENT = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36'

function clientSuffix() {
  return {
    clientType: PC,
    version: VERSION,
    channelId: CHANNEL_ID,
    rand: `${Math.floor(Math.random() * 100000)}_${Math.floor(Math.random() * 10000000000)}`,
  }
}

function requestPath(fullUrl) {
  return new URL(fullUrl).pathname || '/'
}

function signatureHeaders(token, method, fullUrl) {
  const sessionKey = token.open_api_access_token || token.sessionKey || token.access_token || ''
  const sessionSecret = token.open_api_refresh_token || token.sessionSecret || token.signature || ''
  const date = new Date().toUTCString()
  const text = `SessionKey=${sessionKey}&Operate=${method.toUpperCase()}&RequestURI=${requestPath(fullUrl)}&Date=${date}`
  return {
    Date: date,
    SessionKey: sessionKey,
    'X-Request-ID': crypto.randomUUID(),
    Signature: createHmac('sha1', sessionSecret).update(text).digest('hex').toUpperCase(),
  }
}

function normalizeToken(token, data) {
  return {
    ...token,
    access_token: data.accessToken || data.sessionKey || token.access_token,
    refresh_token: data.refreshToken || token.refresh_token,
    open_api_access_token: data.sessionKey || token.open_api_access_token,
    open_api_refresh_token: data.sessionSecret || token.open_api_refresh_token,
    signature: data.sessionSecret || token.signature,
    expire_time: data.expiresIn ? new Date(Date.now() + Number(data.expiresIn) * 1000).toISOString() : token.expire_time,
  }
}

export async function cloud189RefreshToken(token) {
  if (!token.refresh_token) return token
  const refreshResp = await fetch(`${AUTH_URL}/api/oauth2/refreshToken.do`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded', 'User-Agent': USER_AGENT },
    body: new URLSearchParams({ clientId: APP_ID, refreshToken: token.refresh_token, grantType: 'refresh_token', format: 'json' }),
  })
  const refreshData = await refreshResp.json().catch(() => undefined)
  if (!refreshResp.ok || !refreshData?.accessToken) {
    const err = new Error(refreshData?.res_message || refreshData?.message || `189 token refresh failed ${refreshResp.status}`)
    err.code = 'ERR_189_AUTH'
    throw err
  }
  const params = new URLSearchParams({ ...clientSuffix(), appId: APP_ID, accessToken: refreshData.accessToken })
  const sessionResp = await fetch(`${API_URL}/getSessionForPC.action?${params.toString()}`, {
    headers: { Accept: 'application/json;charset=UTF-8', 'User-Agent': USER_AGENT },
  })
  const sessionData = await sessionResp.json().catch(() => undefined)
  if (!sessionResp.ok || Number(sessionData?.res_code || 0) !== 0) {
    const err = new Error(sessionData?.res_message || sessionData?.message || `189 session refresh failed ${sessionResp.status}`)
    err.code = 'ERR_189_AUTH'
    throw err
  }
  return normalizeToken(token, { ...refreshData, ...sessionData, refreshToken: refreshData.refreshToken || token.refresh_token })
}

async function requestJson(token, method, action, params = {}) {
  const url = `${API_URL}/${action}`
  const query = new URLSearchParams({ ...clientSuffix(), ...params }).toString()
  const resp = await fetch(`${url}?${query}`, {
    method,
    headers: {
      Accept: 'application/json;charset=UTF-8',
      'User-Agent': USER_AGENT,
      Referer: WEB_URL,
      ...signatureHeaders(token, method, url),
    },
  })
  const data = await resp.json().catch(() => undefined)
  if (!resp.ok || ![undefined, '', 0, '0'].includes(data?.res_code) || (data?.code && data.code !== 'SUCCESS')) {
    const err = new Error(data?.res_message || data?.message || `189 API ${resp.status}`)
    err.code = data?.code || data?.res_code || 'ERR_189_API'
    err.status = resp.status
    throw err
  }
  return data
}

async function requestForm(token, action, form) {
  const url = `${API_URL}/${action}`
  const query = new URLSearchParams(clientSuffix()).toString()
  const resp = await fetch(`${url}?${query}`, {
    method: 'POST',
    headers: {
      Accept: 'application/json;charset=UTF-8',
      'Content-Type': 'application/x-www-form-urlencoded',
      'User-Agent': USER_AGENT,
      Referer: WEB_URL,
      ...signatureHeaders(token, 'POST', url),
    },
    body: new URLSearchParams(form),
  })
  const data = await resp.json().catch(() => undefined)
  if (!resp.ok || ![undefined, '', 0, '0'].includes(data?.res_code) || (data?.code && data.code !== 'SUCCESS')) {
    const err = new Error(data?.res_message || data?.message || `189 API ${resp.status}`)
    err.code = data?.code || data?.res_code || 'ERR_189_API'
    err.status = resp.status
    throw err
  }
  return data
}

function parentId(id) {
  return !id || id === 'cloud189_root' || id === '0' || id === '/' ? '-11' : String(id)
}

function mapFolder(item, accountId, parent = 'cloud189_root') {
  return {
    provider: '189',
    accountId,
    driveId: 'cloud189',
    fileId: String(item.id || item.folderId || ''),
    parentFileId: String(item.parentId || item.parentFolderId || parent),
    name: item.name || item.folderName || '',
    type: 'folder',
    size: 0,
    updatedAt: item.lastOpTime,
    createdAt: item.createDate,
  }
}

function mapFile(item, accountId, parent = 'cloud189_root') {
  return {
    provider: '189',
    accountId,
    driveId: 'cloud189',
    fileId: String(item.id || item.fileId || ''),
    parentFileId: String(item.parentId || item.parentFolderId || parent),
    name: item.name || item.fileName || '',
    type: 'file',
    size: Number(item.size || 0),
    updatedAt: item.lastOpTime,
    createdAt: item.createDate,
  }
}

export async function cloud189ListDir(token, currentParentId = 'cloud189_root', { limit = 1000 } = {}) {
  const data = await requestJson(token, 'GET', 'listFiles.action', {
    folderId: parentId(currentParentId),
    fileType: '0',
    mediaAttr: '0',
    iconOption: '5',
    pageNum: '1',
    pageSize: String(limit),
    recursive: '0',
    orderBy: 'filename',
    descending: 'false',
  })
  const ao = data.fileListAO || data?.data?.fileListAO || {}
  const folders = Array.isArray(ao.folderList) ? ao.folderList.map((item) => mapFolder(item, token.user_id, currentParentId)) : []
  const files = Array.isArray(ao.fileList) ? ao.fileList.map((item) => mapFile(item, token.user_id, currentParentId)) : []
  return [...folders, ...files]
}

export async function* cloud189Walk(token, currentParentId = 'cloud189_root', maxDepth = 10) {
  const queue = [{ parentFileId: currentParentId, depth: 0 }]
  while (queue.length) {
    const current = queue.shift()
    const items = await cloud189ListDir(token, current.parentFileId)
    for (const item of items) {
      yield item
      if (item.type === 'folder' && current.depth < maxDepth) queue.push({ parentFileId: item.fileId, depth: current.depth + 1 })
    }
  }
}

export async function cloud189GetFile(token, fileId) {
  if (fileId === 'cloud189_root' || fileId === '-11' || fileId === '0') return { provider: '189', accountId: token.user_id, driveId: 'cloud189', fileId: 'cloud189_root', parentFileId: '', name: '网盘文件', type: 'folder' }
  const items = await cloud189ListDir(token, 'cloud189_root')
  const item = items.find((entry) => entry.fileId === String(fileId))
  if (item) return item
  const err = new Error(`File not found: ${fileId}`)
  err.code = 'ERR_189_NOT_FOUND'
  throw err
}

export async function cloud189RenameBatch(token, renames) {
  const results = []
  for (const { fileId, newName } of renames) {
    try {
      await requestJson(token, 'POST', 'renameFile.action', { fileId, destFileName: newName })
      results.push({ fileId, status: 'success', newName })
    } catch {
      try {
        await requestJson(token, 'POST', 'renameFolder.action', { folderId: fileId, destFolderName: newName })
        results.push({ fileId, status: 'success', newName })
      } catch (e) {
        results.push({ fileId, status: 'error', code: e.code || 'ERR', message: e.message })
      }
    }
  }
  return results
}

async function createBatchTask(token, type, ids, targetFolderId = '') {
  const taskInfos = ids.map((id) => ({ fileId: String(id), fileName: '', isFolder: 0 }))
  const data = await requestForm(token, 'batch/createBatchTask.action', { type, taskInfos: JSON.stringify(taskInfos), targetFolderId })
  return String(data.taskId || data.task_id || '')
}

export async function cloud189MoveBatch(token, moves) {
  const groups = new Map()
  for (const move of moves) {
    const key = parentId(move.toParentId)
    if (!groups.has(key)) groups.set(key, [])
    groups.get(key).push(move)
  }
  const results = []
  for (const [target, group] of groups) {
    try {
      const taskId = await createBatchTask(token, 'MOVE', group.map((item) => item.fileId), target)
      if (taskId) await requestForm(token, 'batch/checkBatchTask.action', { taskId })
      results.push(...group.map(({ fileId }) => ({ fileId, status: 'success' })))
    } catch (e) {
      results.push(...group.map(({ fileId }) => ({ fileId, status: 'error', code: e.code || 'ERR', message: e.message })))
    }
  }
  return results
}

export async function cloud189Mkdir(token, currentParentId = 'cloud189_root', name) {
  const data = await requestJson(token, 'POST', 'createFolder.action', { parentFolderId: parentId(currentParentId), folderName: name, relativePath: '' })
  const fileId = data.folderId || data.id || data?.data?.folderId || ''
  return { provider: '189', accountId: token.user_id, driveId: 'cloud189', fileId: String(fileId), parentFileId: String(currentParentId), name, type: 'folder' }
}

export async function cloud189Trash(token, fileIds) {
  if (!fileIds.length) return []
  const taskId = await createBatchTask(token, 'DELETE', fileIds)
  if (taskId) await requestForm(token, 'batch/checkBatchTask.action', { taskId })
  return fileIds.map((fileId) => ({ fileId, status: 'success' }))
}
