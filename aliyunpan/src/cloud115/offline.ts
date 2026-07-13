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

type Drive115OfflineTask = {
  info_hash?: string
  percentDone?: number
  size?: number
  name?: string
  status?: number
}

const getToken = (user_id: string) => UserDAL.GetUserToken(user_id)?.access_token

const formRequest = async (user_id: string, path: string, fields: Record<string, string>) => {
  const accessToken = getToken(user_id)
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
    if (dirID) fields.wp_path_id = dirID.includes('root') ? '0' : dirID
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
  const accessToken = getToken(user_id)
  if (!accessToken) return { ...result, error: '请先登录 115 网盘' }
  try {
    const params = new URLSearchParams({ page: '1' })
    const resp = await fetch(`${API_BASE}/get_task_list?${params}`, { headers: { Authorization: `Bearer ${accessToken}` } })
    const data = await resp.json().catch(() => ({}))
    if (!resp.ok || data?.code !== 0 || data?.state === false) {
      return { ...result, error: data?.message || '获取 115 云下载进度失败' }
    }
    const tasks: Drive115OfflineTask[] = data?.data?.tasks || []
    const task = tasks.find(item => String(item.info_hash || '') === String(taskId))
    if (!task) return { ...result, status: 2, process: 100 }
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

export const apiDrive115OfflineDelete = async (user_id: string, taskIds: string[]) => {
  for (const taskId of taskIds.filter(Boolean)) {
    try {
      await formRequest(user_id, 'del_task', { info_hash: taskId, del_source_file: '0' })
    } catch {
      // Continue deleting the remaining tasks.
    }
  }
}
