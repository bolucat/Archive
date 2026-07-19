import { guangyaApiParentId, guangyaRequest } from './dirfilelist'

export interface GuangyaOfflineCreateResult {
  taskId: string
  fileId: string
  error: string
}

export interface GuangyaOfflineProcessResult {
  status: number
  process: number
  error: string
}

const normalizeTask = (item: any) => ({
  taskId: String(item?.taskId || item?.id || item?.task_id || ''),
  fileId: String(item?.fileId || item?.file_id || item?.resId || ''),
  status: Number(item?.status ?? item?.taskStatus ?? 0),
  process: Number(item?.process ?? item?.progress ?? item?.percent ?? 0)
})

const VIDEO_EXT = /\.(mp4|mkv|avi|mov|wmv|flv|webm|m4v|mpg|mpeg|3gp|rmvb|ts|m2ts|mts|vob)$/i

const resolveOfflineResource = async (user_id: string, url: string) => {
  const data = await guangyaRequest(user_id, '/cloudcollection/v1/resolve_res', { url })
  return data?.data || data || {}
}

export const apiGuangyaOfflineCreate = async (user_id: string, url: string, fileName: string, dirID: string | undefined): Promise<GuangyaOfflineCreateResult> => {
  try {
    const resolved = /^(?:magnet:|ed2k:)/i.test(url) ? await resolveOfflineResource(user_id, url) : { url }
    const subfiles = resolved?.btResInfo?.subfiles || resolved?.btResInfo?.subFiles || []
    const videoIndexes = Array.isArray(subfiles)
      ? subfiles.map((item: any, index: number) => ({ index: Number.isInteger(item?.fileIndex) ? item.fileIndex : index, name: String(item?.fileName || item?.name || '') })).filter((item: any) => VIDEO_EXT.test(item.name)).map((item: any) => item.index)
      : []
    const body: Record<string, unknown> = {
      url: resolved?.url || url,
      parentId: guangyaApiParentId(dirID === 'guangya' ? 'guangya_root' : dirID),
      newName: resolved?.btResInfo?.fileName || fileName || 'offline'
    }
    if (videoIndexes.length) body.fileIndexes = videoIndexes
    const data = await guangyaRequest(user_id, '/cloudcollection/v1/create_task', body)
    const responseBody = data?.data || data || {}
    const task = normalizeTask(responseBody)
    return {
      taskId: task.taskId,
      fileId: task.fileId,
      error: task.taskId || task.fileId ? '' : (data?.message || data?.msg || '创建光鸭云盘离线下载失败')
    }
  } catch (error: any) {
    return { taskId: '', fileId: '', error: error?.message || '创建光鸭云盘离线下载失败' }
  }
}

export const apiGuangyaOfflineProcess = async (user_id: string, taskId: string): Promise<GuangyaOfflineProcessResult> => {
  try {
    const data = await guangyaRequest(user_id, '/cloudcollection/v1/list_task', { taskIds: [taskId] })
    const body = data?.data || data || {}
    const list = body?.list || body?.items || body?.records || body?.content || []
    const raw = Array.isArray(list) ? list.find((item) => String(item?.taskId || item?.id || item?.task_id || '') === String(taskId)) : undefined
    if (!raw) return { status: 2, process: 100, error: '' }
    const task = normalizeTask(raw)
    const process = task.process <= 1 && task.process > 0 ? Math.round(task.process * 100) : task.process
    return { status: task.status, process, error: '' }
  } catch (error: any) {
    return { status: 0, process: 0, error: error?.message || '获取光鸭云盘离线下载进度失败' }
  }
}
