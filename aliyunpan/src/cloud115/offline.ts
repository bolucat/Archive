import UserDAL from '../user/userdal'

const API_BASE = 'https://proapi.115.com/open/offline'

export type Drive115OfflineCreateResult = {
  taskIds: string[]
  error: string
}

export type Drive115OfflineProcessResult = {
  process: number
  status: number
  name: string
  size: number
  error: string
}

export type Drive115OfflineQuota = {
  surplus?: number
  used?: number
  count?: number
  name?: string
  expire_time?: number
  details?: Drive115OfflineQuota[]
}

export type Drive115TorrentFile = {
  index: number
  name: string
  size: number
  wanted: boolean
}

export type Drive115OfflineTask = {
  info_hash?: string
  url?: string
  percentDone?: number
  size?: number
  name?: string
  status?: number
}

const getToken = async (user_id: string) => {
  const token = await UserDAL.EnsureUserTokenReady(user_id)
  return token?.access_token || ''
}

const formRequest = async (user_id: string, path: string, fields: Record<string, string>) => {
  const accessToken = await getToken(user_id)
  if (!accessToken) throw new Error('请先登录 115 网盘')
  const body = new URLSearchParams(fields)
  const resp = await fetch(`${API_BASE}/${path}`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/x-www-form-urlencoded'
    },
    body
  })
  const data = await resp.json().catch(() => ({}))
  if (!resp.ok || data?.code !== 0 || data?.state === false) {
    throw new Error(data?.message || '115 云下载请求失败')
  }
  return data
}

export const apiDrive115OfflineCreate = async (user_id: string, url: string, dirID?: string): Promise<Drive115OfflineCreateResult> => {
  try {
    const fields: Record<string, string> = { urls: url }
    if (dirID) fields.wp_path_id = dirID === 'drive115' || dirID.includes('root') ? '0' : dirID
    const data = await formRequest(user_id, 'add_task_urls', fields)
    const items = Array.isArray(data?.data) ? data.data : []
    const taskIds = items
      .filter((item: any) => item?.state !== false && item?.code === 0)
      .map((item: any) => String(item?.info_hash || ''))
      .filter(Boolean)
    return { taskIds, error: taskIds.length ? '' : (data?.message || '115 云下载任务未创建') }
  } catch (error: any) {
    return { taskIds: [], error: error?.message || '创建 115 云下载失败' }
  }
}

export const apiDrive115OfflineProcess = async (user_id: string, taskId: string): Promise<Drive115OfflineProcessResult> => {
  const result: Drive115OfflineProcessResult = { process: 0, status: 0, name: '', size: 0, error: '' }
  const accessToken = await getToken(user_id)
  if (!accessToken) return { ...result, error: '请先登录 115 网盘' }
  try {
    const first = await fetch115OfflineTasks(accessToken, 1)
    if (first.error) return { ...result, error: first.error }
    let tasks = first.tasks
    const pageCount = Math.max(1, Number(first.pageCount) || 1)
    for (let page = 2; page <= pageCount; page++) {
      const next = await fetch115OfflineTasks(accessToken, page)
      if (next.error) return { ...result, error: next.error }
      tasks = tasks.concat(next.tasks)
    }
    const task = tasks.find(item => String(item.info_hash || '') === String(taskId))
    if (!task) return { ...result, error: '115 云下载任务不存在' }
    return {
      process: Math.max(0, Math.min(100, Number(task.percentDone) || 0)),
      status: Number(task.status) || 0,
      name: task.name || '',
      size: Number(task.size) || 0,
      error: ''
    }
  } catch (error: any) {
    return { ...result, error: error?.message || '获取 115 云下载进度失败' }
  }
}

const fetch115OfflineTasks = async (accessToken: string, page: number) => {
  const params = new URLSearchParams({ page: String(page) })
  const resp = await fetch(`${API_BASE}/get_task_list?${params}`, { headers: { Authorization: `Bearer ${accessToken}` } })
  const data = await resp.json().catch(() => ({}))
  if (!resp.ok || data?.code !== 0 || data?.state === false) {
    return { tasks: [] as Drive115OfflineTask[], pageCount: 0, error: data?.message || '获取 115 云下载进度失败' }
  }
  return { tasks: (data?.data?.tasks || []) as Drive115OfflineTask[], pageCount: Number(data?.data?.page_count) || 1, error: '' }
}

export const apiDrive115OfflineTasks = async (user_id: string) => {
  const accessToken = await getToken(user_id)
  if (!accessToken) return { tasks: [] as Drive115OfflineTask[], error: '' }
  try {
    const first = await fetch115OfflineTasks(accessToken, 1)
    if (first.error) return { tasks: [], error: first.error }
    let tasks = first.tasks
    for (let page = 2; page <= Math.max(1, first.pageCount); page++) {
      const next = await fetch115OfflineTasks(accessToken, page)
      if (next.error) return { tasks, error: next.error }
      tasks = tasks.concat(next.tasks)
    }
    return { tasks, error: '' }
  } catch (error: any) {
    return { tasks: [], error: error?.message || '获取 115 云下载任务失败' }
  }
}

export const apiDrive115OfflineQuota = async (user_id: string): Promise<{ items: Drive115OfflineQuota[]; error: string }> => {
  const accessToken = await getToken(user_id)
  if (!accessToken) return { items: [], error: '请先登录 115 网盘' }
  try {
    const resp = await fetch(`${API_BASE}/get_quota_info`, { headers: { Authorization: `Bearer ${accessToken}` } })
    const data = await resp.json().catch(() => ({}))
    if (!resp.ok || data?.code !== 0 || data?.state === false) return { items: [], error: data?.message || '获取 115 云下载配额失败' }
    const raw = data?.data
    const items = Array.isArray(raw) ? raw : Array.isArray(raw?.list) ? raw.list : raw ? [raw] : []
    return { items, error: '' }
  } catch (error: any) {
    return { items: [], error: error?.message || '获取 115 云下载配额失败' }
  }
}

export const apiDrive115OfflineClear = async (user_id: string, flag: number): Promise<{ success: boolean; error: string }> => {
  try {
    await formRequest(user_id, 'clear_task', { flag: String(flag) })
    return { success: true, error: '' }
  } catch (error: any) {
    return { success: false, error: error?.message || '清空 115 云下载任务失败' }
  }
}

export const apiDrive115TorrentParse = async (user_id: string, torrentSha1: string, pickCode: string) => {
  try {
    const data = await formRequest(user_id, 'torrent', { torrent_sha1: torrentSha1, pick_code: pickCode })
    const files = Array.isArray(data?.data?.torrent_filelist) ? data.data.torrent_filelist : []
    return {
      infoHash: String(data?.data?.info_hash || ''),
      name: String(data?.data?.torrent_name || ''),
      files: files.map((item: any, index: number) => ({ index, name: String(item?.path || item?.name || ''), size: Number(item?.size || item?.file_size || 0), wanted: item?.wanted !== 0 })),
      error: ''
    }
  } catch (error: any) {
    return { infoHash: '', name: '', files: [] as Drive115TorrentFile[], error: error?.message || '解析 BT 种子失败' }
  }
}

export const apiDrive115OfflineAddTorrent = async (user_id: string, params: { infoHash: string; wanted: string; savePath: string; torrentSha1: string; pickCode: string; dirID?: string }) => {
  try {
    const fields: Record<string, string> = {
      info_hash: params.infoHash,
      wanted: params.wanted,
      save_path: params.savePath,
      torrent_sha1: params.torrentSha1,
      pick_code: params.pickCode
    }
    if (params.dirID) fields.wp_path_id = params.dirID === 'drive115' || params.dirID.includes('root') ? '0' : params.dirID
    const data = await formRequest(user_id, 'add_task_bt', fields)
    return { success: true, error: data?.message || '' }
  } catch (error: any) {
    return { success: false, error: error?.message || '创建 115 BT 云下载任务失败' }
  }
}

export const apiDrive115OfflineDelete = async (user_id: string, taskIds: string[]) => {
  for (const taskId of taskIds.filter(Boolean)) {
    try {
      await formRequest(user_id, 'del_task', { info_hash: taskId, del_source_file: '0' })
    } catch {
      // Continue deleting the remaining tasks.
    }
  }
}
