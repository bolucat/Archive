const BASE = 'https://drive-pc.quark.cn/1/clouddrive'
const USER_AGENT = 'Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) quark-cloud-drive/2.5.56 Chrome/100.0.4896.143 Electron/18.3.5.12-a038f7b798 Safari/537.36 Channel/pckk_other_ch'

function quarkCookie(token) {
  return token.access_token || token.cookie || token.cookies || ''
}

function headers(token) {
  return {
    Accept: 'application/json, text/plain, */*',
    'Content-Type': 'application/json',
    Origin: 'https://pan.quark.cn',
    Referer: 'https://pan.quark.cn/',
    'User-Agent': USER_AGENT,
    Cookie: quarkCookie(token),
  }
}

function params(extra = {}) {
  const qs = new URLSearchParams({
    pr: 'ucpro',
    fr: 'pc',
    uc_param_str: '',
    __t: String(Date.now()),
    __dt: '1000',
  })
  for (const [key, value] of Object.entries(extra)) {
    if (value !== undefined && value !== '') qs.set(key, String(value))
  }
  return qs
}

async function quarkRequest(path, token, { method = 'GET', body, query } = {}) {
  const resp = await fetch(`${BASE}/${path.replace(/^\//, '')}?${params(query).toString()}`, {
    method,
    headers: headers(token),
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  const data = await resp.json().catch(() => undefined)
  if (!resp.ok || data?.status === 'error' || (data?.code && data.code !== 0)) {
    const err = new Error(data?.message || `Quark API ${resp.status}`)
    err.code = data?.code || 'ERR_QUARK_API'
    err.status = resp.status
    throw err
  }
  return data
}

function normalizeRoot(parentFileId) {
  return !parentFileId || parentFileId === 'quark_root' ? '0' : String(parentFileId)
}

function listFromResponse(data) {
  const list = data?.data?.list || data?.data || []
  return Array.isArray(list) ? list : []
}

function mapItem(item, accountId, parentFileId = 'quark_root') {
  const isFolder = Number(item.file_type || 0) === 0
  return {
    provider: 'quark',
    accountId,
    driveId: 'quark',
    fileId: String(item.fid || ''),
    parentFileId: String(item.pdir_fid || parentFileId || 'quark_root'),
    name: item.file_name || '',
    type: isFolder ? 'folder' : 'file',
    size: Number(item.size || 0),
    updatedAt: item.updated_at ? new Date(Number(item.updated_at) * (Number(item.updated_at) > 100000000000 ? 1 : 1000)).toISOString() : undefined,
    createdAt: item.created_at ? new Date(Number(item.created_at) * (Number(item.created_at) > 100000000000 ? 1 : 1000)).toISOString() : undefined,
  }
}

export async function quarkRefreshToken(token) {
  return token
}

export async function quarkListDir(token, parentFileId = 'quark_root', { limit = 200 } = {}) {
  const parent = normalizeRoot(parentFileId)
  const data = await quarkRequest('file/sort', token, {
    query: { pdir_fid: parent, _page: 1, _size: limit, _sort: 'file_name:asc' },
  })
  return listFromResponse(data).map((item) => mapItem(item, token.user_id, parentFileId))
}

export async function* quarkWalk(token, parentFileId = 'quark_root', maxDepth = 10) {
  const queue = [{ parentFileId, depth: 0 }]
  while (queue.length) {
    const current = queue.shift()
    const items = await quarkListDir(token, current.parentFileId)
    for (const item of items) {
      yield item
      if (item.type === 'folder' && current.depth < maxDepth) queue.push({ parentFileId: item.fileId, depth: current.depth + 1 })
    }
  }
}

export async function quarkSearch(token, keyword, { limit = 100 } = {}) {
  const data = await quarkRequest('file/search', token, {
    query: { q: keyword, _page: 1, _size: limit, _fetch_total: 1, _sort: 'file_type:desc,updated_at:desc', _is_hl: 1 },
  })
  return listFromResponse(data).map((item) => mapItem(item, token.user_id, 'quark_root')).slice(0, limit)
}

export async function quarkGetFile(token, fileId) {
  if (fileId === 'quark_root' || fileId === '0') return { provider: 'quark', accountId: token.user_id, driveId: 'quark', fileId: 'quark_root', parentFileId: '', name: '网盘文件', type: 'folder' }
  const data = await quarkRequest('file', token, { query: { fids: fileId } })
  const item = listFromResponse(data).find((entry) => String(entry.fid) === String(fileId)) || listFromResponse(data)[0]
  if (!item) {
    const err = new Error(`File not found: ${fileId}`)
    err.code = 'ERR_QUARK_NOT_FOUND'
    throw err
  }
  return mapItem(item, token.user_id, item.pdir_fid || 'quark_root')
}

export async function quarkRenameBatch(token, renames) {
  const results = []
  for (const { fileId, newName } of renames) {
    try {
      await quarkRequest('file/rename', token, { method: 'POST', body: { fid: fileId, file_name: newName } })
      results.push({ fileId, status: 'success', newName })
    } catch (e) {
      results.push({ fileId, status: 'error', code: e.code || 'ERR', message: e.message })
    }
  }
  return results
}

export async function quarkMoveBatch(token, moves) {
  const groups = new Map()
  for (const move of moves) {
    const key = normalizeRoot(move.toParentId)
    if (!groups.has(key)) groups.set(key, [])
    groups.get(key).push(move)
  }
  const results = []
  for (const [toParentId, group] of groups) {
    try {
      await quarkRequest('file/move', token, {
        method: 'POST',
        body: { action_type: 1, to_pdir_fid: toParentId, filelist: group.map((item) => item.fileId), exclude_fids: [] },
      })
      results.push(...group.map(({ fileId }) => ({ fileId, status: 'success' })))
    } catch (e) {
      results.push(...group.map(({ fileId }) => ({ fileId, status: 'error', code: e.code || 'ERR', message: e.message })))
    }
  }
  return results
}

export async function quarkMkdir(token, parentId = 'quark_root', name) {
  const data = await quarkRequest('file', token, {
    method: 'POST',
    body: { pdir_fid: normalizeRoot(parentId), file_name: name, dir_init_lock: false, dir_path: '' },
  })
  const fileId = data?.data?.fid || data?.data?.file?.fid || ''
  return { provider: 'quark', accountId: token.user_id, driveId: 'quark', fileId: String(fileId), parentFileId: String(parentId), name, type: 'folder' }
}

export async function quarkTrash(token, fileIds) {
  if (fileIds.length === 0) return []
  await quarkRequest('file/delete', token, {
    method: 'POST',
    body: { action_type: 2, filelist: fileIds, exclude_fids: [] },
  })
  return fileIds.map((fileId) => ({ fileId, status: 'success' }))
}
