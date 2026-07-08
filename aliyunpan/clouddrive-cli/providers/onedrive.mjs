const BASE = 'https://graph.microsoft.com/v1.0'
const TOKEN_URL = 'https://login.microsoftonline.com/common/oauth2/v2.0/token'
const CLIENT_ID = ''
const SMALL_UPLOAD_LIMIT = 250 * 1024 * 1024
const SESSION_CHUNK_SIZE = 10 * 1024 * 1024

import { open } from 'node:fs/promises'
import { readFileBuffer, readSlice, toConflictMode } from './uploadUtils.mjs'
import { downloadResponseToFile } from '../core/downloadFile.mjs'

function odHeaders(token) {
  return { 'Content-Type': 'application/json', 'Authorization': `Bearer ${token.access_token}` }
}

async function odGet(url, token) {
  const resp = await fetch(url, { headers: odHeaders(token) })
  if (!resp.ok) {
    const text = await resp.text().catch(() => '')
    const err = new Error(`OneDrive API ${resp.status}: ${text.slice(0, 200)}`)
    err.code = 'ERR_ONEDRIVE_HTTP'
    err.status = resp.status
    throw err
  }
  return resp.json()
}

async function odPatch(url, body, token) {
  const resp = await fetch(url, { method: 'PATCH', headers: odHeaders(token), body: JSON.stringify(body) })
  if (!resp.ok) {
    const text = await resp.text().catch(() => '')
    const err = new Error(`OneDrive API ${resp.status}: ${text.slice(0, 200)}`)
    err.code = 'ERR_ONEDRIVE_HTTP'
    throw err
  }
  return resp.json()
}

async function odPost(url, body, token) {
  const resp = await fetch(url, { method: 'POST', headers: odHeaders(token), body: JSON.stringify(body) })
  if (!resp.ok) {
    const text = await resp.text().catch(() => '')
    const err = new Error(`OneDrive API ${resp.status}: ${text.slice(0, 200)}`)
    err.code = 'ERR_ONEDRIVE_HTTP'
    throw err
  }
  return resp.json()
}

async function odDelete(url, token) {
  const resp = await fetch(url, { method: 'DELETE', headers: { 'Authorization': `Bearer ${token.access_token}` } })
  if (!resp.ok && resp.status !== 204) {
    const text = await resp.text().catch(() => '')
    const err = new Error(`OneDrive API ${resp.status}: ${text.slice(0, 200)}`)
    err.code = 'ERR_ONEDRIVE_HTTP'
    throw err
  }
}

export async function onedriveRefreshToken(token) {
  const clientId = token.device_id || CLIENT_ID
  const params = new URLSearchParams({
    client_id: clientId,
    scope: 'offline_access Files.ReadWrite User.Read',
    refresh_token: token.refresh_token,
    grant_type: 'refresh_token',
  })
  const resp = await fetch(TOKEN_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: params.toString(),
  })
  if (!resp.ok) {
    const text = await resp.text().catch(() => '')
    const err = new Error(`OneDrive token refresh failed ${resp.status}: ${text.slice(0, 200)}`)
    err.code = 'ERR_ONEDRIVE_AUTH'
    throw err
  }
  const data = await resp.json()
  return { ...token, access_token: data.access_token, refresh_token: data.refresh_token || token.refresh_token, expires_in: data.expires_in, token_type: data.token_type || 'Bearer' }
}

function mapFileItem(item, accountId) {
  const isFolder = !!item.folder
  return {
    provider: 'onedrive',
    accountId,
    driveId: 'onedrive',
    fileId: item.id,
    parentFileId: item.parentReference?.id || 'onedrive_root',
    name: item.name,
    type: isFolder ? 'folder' : 'file',
    size: item.size,
    contentHash: item.file?.hashes?.quickXorHash,
    mimeType: item.file?.mimeType,
    updatedAt: item.lastModifiedDateTime,
    createdAt: item.createdDateTime,
  }
}

export async function onedriveListDir(token, parentFileId = 'onedrive_root') {
  const accountId = token.user_id
  const allItems = []
  const isRoot = parentFileId === 'onedrive_root' || !parentFileId
  let url = isRoot
    ? `${BASE}/me/drive/root/children?$expand=thumbnails`
    : `${BASE}/me/drive/items/${parentFileId}/children?$expand=thumbnails`

  while (url) {
    const data = await odGet(url, token)
    for (const item of data.value || []) allItems.push(mapFileItem(item, accountId))
    url = data['@odata.nextLink'] || ''
  }
  return allItems
}

export async function* onedriveWalk(token, parentFileId = 'onedrive_root', maxDepth = 10) {
  const queue = [{ parentFileId, depth: 0 }]
  while (queue.length > 0) {
    const { parentFileId: pid, depth } = queue.shift()
    const items = await onedriveListDir(token, pid)
    for (const item of items) {
      yield item
      if (item.type === 'folder' && depth < maxDepth) {
        queue.push({ parentFileId: item.fileId, depth: depth + 1 })
      }
    }
  }
}

export async function onedriveSearch(token, keyword, { limit = 100 } = {}) {
  const accountId = token.user_id
  const allItems = []
  const seen = new Set()
  const q = encodeURIComponent(keyword)
  let url = `${BASE}/me/drive/root/search(q='${q}')?$top=${Math.min(limit, 200)}`
  while (url && allItems.length < limit) {
    const data = await odGet(url, token)
    for (const item of data.value || []) {
      if (!item?.id || seen.has(item.id)) continue
      seen.add(item.id)
      allItems.push(mapFileItem(item, accountId))
    }
    url = data['@odata.nextLink'] || ''
  }
  return allItems.slice(0, limit)
}

export async function onedriveRenameBatch(token, renames) {
  const results = []
  for (const { fileId, newName } of renames) {
    try {
      const data = await odPatch(`${BASE}/me/drive/items/${fileId}`, { name: newName }, token)
      results.push({ fileId, status: 'success', newName: data.name || newName })
    } catch (e) {
      results.push({ fileId, status: 'error', code: e.code || 'ERR', message: e.message })
    }
  }
  return results
}

export async function onedriveGetFile(token, fileId) {
  const accountId = token.user_id
  const data = await odGet(`${BASE}/me/drive/items/${fileId}`, token)
  return mapFileItem(data, accountId)
}

export async function onedriveDownloadFile(token, fileId, outputPath) {
  const meta = await odGet(`${BASE}/me/drive/items/${fileId}`, token)
  if (meta.folder) {
    const err = new Error(`Cannot download folder: ${fileId}`)
    err.code = 'ERR_ONEDRIVE_DOWNLOAD_FOLDER'
    throw err
  }
  const resp = await fetch(`${BASE}/me/drive/items/${fileId}/content`, {
    headers: { Authorization: `Bearer ${token.access_token}` },
    redirect: 'follow',
  })
  await downloadResponseToFile(resp, outputPath)
  return { ok: true, provider: 'onedrive', accountId: token.user_id, driveId: 'onedrive', fileId, name: meta.name || '', size: meta.size || 0, output: outputPath }
}

export async function onedriveMove(token, moves) {
  const results = []
  for (const { fileId, toParentId } of moves) {
    try {
      const data = await odPatch(`${BASE}/me/drive/items/${fileId}`, { parentReference: { id: toParentId } }, token)
      results.push({ fileId, status: 'success', newParentId: data.parentReference?.id || toParentId })
    } catch (e) {
      results.push({ fileId, status: 'error', code: e.code || 'ERR', message: e.message })
    }
  }
  return results
}

export async function onedriveMkdir(token, parentId, name) {
  const accountId = token.user_id
  const isRoot = parentId === 'onedrive_root' || !parentId
  const url = isRoot
    ? `${BASE}/me/drive/root/children`
    : `${BASE}/me/drive/items/${parentId}/children`
  const data = await odPost(url, { name, folder: {}, '@microsoft.graph.conflictBehavior': 'rename' }, token)
  return mapFileItem(data, accountId)
}

export async function onedriveTrash(token, fileIds) {
  const results = []
  for (const fileId of fileIds) {
    try {
      await odDelete(`${BASE}/me/drive/items/${fileId}`, token)
      results.push({ fileId, status: 'success' })
    } catch (e) {
      results.push({ fileId, status: 'error', code: e.code || 'ERR', message: e.message })
    }
  }
  return results
}

function encodePathSegment(value) {
  return encodeURIComponent(value).replace(/%2F/g, '/')
}

function onedriveConflict(conflict) {
  const mode = toConflictMode(conflict)
  if (mode === 'auto_rename') return 'rename'
  if (mode === 'overwrite') return 'replace'
  return 'fail'
}

function onedriveUploadPath(parentId, name, suffix) {
  const encoded = encodePathSegment(name)
  if (!parentId || parentId === 'onedrive_root') return `${BASE}/me/drive/root:/${encoded}:/${suffix}`
  return `${BASE}/me/drive/items/${encodeURIComponent(parentId)}:/${encoded}:/${suffix}`
}

async function putOneDriveUploadUrl(url, body, start, total) {
  const end = start + body.length - 1
  const resp = await fetch(url, {
    method: 'PUT',
    headers: {
      'Content-Length': String(body.length),
      'Content-Range': `bytes ${start}-${end}/${total}`,
    },
    body,
  })
  const data = await resp.json().catch(() => undefined)
  if (!resp.ok || data?.error) {
    const err = new Error(data?.error?.message || `OneDrive upload failed ${resp.status}`)
    err.code = 'ERR_ONEDRIVE_UPLOAD'
    throw err
  }
  return data
}

export async function onedriveUploadFile(token, { parentId = 'onedrive_root', localPath, name, size = 0, conflict = 'skip' }) {
  const accountId = token.user_id
  if (size <= SMALL_UPLOAD_LIMIT) {
    const body = await readFileBuffer(localPath)
    const resp = await fetch(onedriveUploadPath(parentId, name, 'content'), {
      method: 'PUT',
      headers: { Authorization: `Bearer ${token.access_token}`, 'Content-Type': 'application/octet-stream' },
      body,
    })
    const data = await resp.json().catch(() => undefined)
    if (!resp.ok || data?.error) {
      const err = new Error(data?.error?.message || `OneDrive upload failed ${resp.status}`)
      err.code = 'ERR_ONEDRIVE_UPLOAD'
      throw err
    }
    return mapFileItem(data, accountId)
  }

  const created = await odPost(onedriveUploadPath(parentId, name, 'createUploadSession'), {
    item: { '@microsoft.graph.conflictBehavior': onedriveConflict(conflict) },
  }, token)
  const uploadUrl = created.uploadUrl
  if (!uploadUrl) {
    const err = new Error('OneDrive createUploadSession returned no uploadUrl')
    err.code = 'ERR_ONEDRIVE_UPLOAD_SESSION'
    throw err
  }
  const handle = await open(localPath, 'r')
  let uploaded
  try {
    for (let offset = 0; offset < size;) {
      const body = await readSlice(handle, offset, Math.min(SESSION_CHUNK_SIZE, size - offset))
      uploaded = await putOneDriveUploadUrl(uploadUrl, body, offset, size)
      offset += body.length
    }
  } finally {
    await handle.close().catch(() => {})
  }
  return mapFileItem(uploaded, accountId)
}
