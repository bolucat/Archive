const BASE = 'https://open-api.123pan.com'
const AUTH_URL = `${BASE}/api/v1/oauth2/access_token`
const CLIENT_ID = ''
const CLIENT_SECRET = ''

import { open } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { buildMultipart, hashFile, readSlice } from './uploadUtils.mjs'

function headers(token) {
  return {
    'Content-Type': 'application/json',
    Platform: 'open_platform',
    Authorization: `Bearer ${token.access_token}`,
  }
}

async function requestJson(url, token, options = {}) {
  const resp = await fetch(url, {
    ...options,
    headers: { ...headers(token), ...(options.headers || {}) },
  })
  if (!resp.ok) {
    const text = await resp.text().catch(() => '')
    const err = new Error(`123 API ${resp.status}: ${text.slice(0, 200)}`)
    err.code = 'ERR_CLOUD123_HTTP'
    err.status = resp.status
    throw err
  }
  const data = await resp.json()
  if (data?.code != null && data.code !== 0) {
    const err = new Error(`123 API error code=${data.code}: ${data.message || ''}`)
    err.code = 'ERR_CLOUD123_API'
    throw err
  }
  return data
}

function mapItem(item, accountId) {
  const isFolder = Number(item.type) === 1
  return {
    provider: 'cloud123',
    accountId,
    driveId: 'cloud123',
    fileId: String(item.fileId || item.fileID || item.file_id || ''),
    parentFileId: String(item.parentFileId || item.parentFileID || item.parent_file_id || '0'),
    name: item.filename || item.fileName || item.name || '',
    type: isFolder ? 'folder' : 'file',
    size: Number(item.size || 0),
    updatedAt: item.updateAt || item.updatedAt,
    createdAt: item.createAt || item.createdAt,
  }
}

function numericIds(ids) {
  return ids.map((id) => Number(id)).filter((id) => !Number.isNaN(id))
}

export async function cloud123RefreshToken(token) {
  const body = new URLSearchParams({
    client_id: CLIENT_ID,
    client_secret: CLIENT_SECRET,
    refresh_token: token.refresh_token,
    grant_type: 'refresh_token',
  })
  const resp = await fetch(AUTH_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body,
  })
  if (!resp.ok) {
    const text = await resp.text().catch(() => '')
    const err = new Error(`123 token refresh failed ${resp.status}: ${text.slice(0, 200)}`)
    err.code = 'ERR_CLOUD123_AUTH'
    throw err
  }
  const data = await resp.json()
  if (!data?.access_token) {
    const err = new Error(`123 token refresh error: ${data?.message || data?.code || 'missing access_token'}`)
    err.code = 'ERR_CLOUD123_AUTH'
    throw err
  }
  return {
    ...token,
    access_token: data.access_token,
    refresh_token: data.refresh_token || token.refresh_token,
    expires_in: data.expires_in || token.expires_in,
    token_type: data.token_type || token.token_type || 'Bearer',
    expire_time: data.expires_in ? new Date(Date.now() + data.expires_in * 1000).toISOString() : token.expire_time,
  }
}

export async function cloud123ListDir(token, parentFileId = '0', { limit = 100, searchData = '', searchMode = 0 } = {}) {
  const params = new URLSearchParams({
    parentFileId: String(parentFileId || '0'),
    limit: String(limit),
    trashed: '0',
  })
  if (searchData) {
    params.set('parentFileId', '0')
    params.set('searchData', searchData)
    params.set('searchMode', String(searchMode))
  }
  const data = await requestJson(`${BASE}/api/v2/file/list?${params}`, token)
  const list = Array.isArray(data?.data?.fileList) ? data.data.fileList : []
  return list.filter((item) => item.trashed !== 1).map((item) => mapItem(item, token.user_id))
}

export async function* cloud123Walk(token, parentFileId = '0', maxDepth = 10) {
  const queue = [{ parentFileId, depth: 0 }]
  while (queue.length > 0) {
    const { parentFileId: currentParent, depth } = queue.shift()
    const items = await cloud123ListDir(token, currentParent)
    for (const item of items) {
      yield item
      if (item.type === 'folder' && depth < maxDepth) queue.push({ parentFileId: item.fileId, depth: depth + 1 })
    }
  }
}

export async function cloud123GetFile(token, fileId) {
  const data = await requestJson(`${BASE}/api/v1/file/detail?fileID=${encodeURIComponent(fileId)}`, token)
  if (!data?.data) {
    const err = new Error(`File not found: ${fileId}`)
    err.code = 'ERR_CLOUD123_NOT_FOUND'
    throw err
  }
  return mapItem(data.data, token.user_id)
}

export async function cloud123Search(token, keyword, { limit = 100 } = {}) {
  return (await cloud123ListDir(token, '0', { limit, searchData: keyword, searchMode: 0 })).slice(0, limit)
}

export async function cloud123RenameBatch(token, renames) {
  const renameList = renames
    .filter((item) => item.fileId && item.newName)
    .map((item) => `${item.fileId}|${item.newName}`)
  if (renameList.length === 0) return []
  try {
    await requestJson(`${BASE}/api/v1/file/rename`, token, {
      method: 'POST',
      body: JSON.stringify({ renameList }),
    })
    return renames.map(({ fileId, newName }) => ({ fileId, status: 'success', newName }))
  } catch (e) {
    return renames.map(({ fileId }) => ({ fileId, status: 'error', code: e.code || 'ERR', message: e.message }))
  }
}

export async function cloud123MoveBatch(token, moves) {
  const groups = new Map()
  for (const move of moves) {
    if (!groups.has(move.toParentId)) groups.set(move.toParentId, [])
    groups.get(move.toParentId).push(move)
  }
  const results = []
  for (const [toParentId, group] of groups) {
    const fileIDs = numericIds(group.map((item) => item.fileId))
    try {
      await requestJson(`${BASE}/api/v1/file/move`, token, {
        method: 'POST',
        body: JSON.stringify({ fileIDs, toParentFileID: Number(toParentId) }),
      })
      results.push(...group.map(({ fileId }) => ({ fileId, status: 'success' })))
    } catch (e) {
      results.push(...group.map(({ fileId }) => ({ fileId, status: 'error', code: e.code || 'ERR', message: e.message })))
    }
  }
  return results
}

export async function cloud123Mkdir(token, parentId = '0', name) {
  const data = await requestJson(`${BASE}/upload/v1/file/mkdir`, token, {
    method: 'POST',
    body: JSON.stringify({ parentID: String(parentId || '0'), name }),
  })
  return {
    provider: 'cloud123',
    accountId: token.user_id,
    driveId: 'cloud123',
    fileId: String(data?.data?.fileID || data?.data?.fileId || data?.data?.file_id || ''),
    parentFileId: String(parentId || '0'),
    name,
    type: 'folder',
  }
}

export async function cloud123Trash(token, fileIds) {
  const fileIDs = numericIds(fileIds)
  if (fileIDs.length === 0) return []
  await requestJson(`${BASE}/api/v1/file/trash`, token, {
    method: 'POST',
    body: JSON.stringify({ fileIDs }),
  })
  return fileIDs.map((fileId) => ({ fileId: String(fileId), status: 'success' }))
}

function normalizeServer(server) {
  if (!server) return ''
  if (server.startsWith('http://') || server.startsWith('https://')) return server
  return `https://${server}`
}

async function upload123Slice(server, token, preuploadID, sliceNo, sliceMD5, body) {
  const form = buildMultipart({ preuploadID, sliceNo: String(sliceNo), sliceMD5 }, {
    name: 'slice',
    filename: 'slice',
    body,
  })
  const resp = await fetch(`${normalizeServer(server)}/upload/v2/file/slice`, {
    method: 'POST',
    headers: {
      'Content-Type': `multipart/form-data; boundary=${form.boundary}`,
      'Content-Length': String(form.body.length),
      Platform: 'open_platform',
      Authorization: `Bearer ${token.access_token}`,
    },
    body: form.body,
  })
  if (!resp.ok) {
    const err = new Error(`123 upload slice failed ${resp.status}`)
    err.code = 'ERR_CLOUD123_UPLOAD_SLICE'
    throw err
  }
}

export async function cloud123UploadFile(token, { parentId = '0', localPath, name, size = 0, conflict = 'skip' }) {
  const { hash: etag } = await hashFile(localPath, 'md5')
  const duplicate = conflict === 'overwrite' ? 2 : 1
  const created = await requestJson(`${BASE}/upload/v2/file/create`, token, {
    method: 'POST',
    body: JSON.stringify({
      parentFileID: Number(parentId || 0),
      filename: name,
      etag,
      size,
      duplicate,
      containDir: false,
    }),
  })
  const data = created.data || {}
  if (data.reuse) {
    return { provider: 'cloud123', accountId: token.user_id, driveId: 'cloud123', fileId: String(data.fileID || data.fileId || ''), parentFileId: String(parentId || '0'), name, type: 'file', size }
  }
  const preuploadID = data.preuploadID || ''
  const sliceSize = Number(data.sliceSize || 0)
  const servers = Array.isArray(data.servers) ? data.servers : []
  if (!preuploadID || !sliceSize || servers.length === 0) {
    const err = new Error('123 upload create returned incomplete upload info')
    err.code = 'ERR_CLOUD123_UPLOAD_CREATE'
    throw err
  }
  const handle = await open(localPath, 'r')
  try {
    let offset = 0
    let sliceNo = 1
    while (offset < size) {
      const body = await readSlice(handle, offset, Math.min(sliceSize, size - offset))
      const sliceMD5 = createHash('md5').update(body).digest('hex')
      await upload123Slice(servers[0], token, preuploadID, sliceNo, sliceMD5, body)
      offset += body.length
      sliceNo += 1
    }
  } finally {
    await handle.close().catch(() => {})
  }

  let completed = null
  for (let i = 0; i < 30; i++) {
    completed = await requestJson(`${BASE}/upload/v2/file/upload_complete`, token, {
      method: 'POST',
      body: JSON.stringify({ preuploadID }),
    })
    if (completed?.data?.completed) break
    await new Promise((resolve) => setTimeout(resolve, 1000))
  }
  const fileID = completed?.data?.fileID || data.fileID || data.fileId
  return { provider: 'cloud123', accountId: token.user_id, driveId: 'cloud123', fileId: String(fileID || ''), parentFileId: String(parentId || '0'), name, type: 'file', size }
}
