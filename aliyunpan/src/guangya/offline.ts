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

export const apiGuangyaOfflineCreate = async (user_id: string, url: string, fileName: string, dirID: string | undefined): Promise<GuangyaOfflineCreateResult> => {
  try {
    const data = await guangyaRequest(user_id, '/nd.bizcloudcollection.s/v1/create_task', {
      url,
      parentId: guangyaApiParentId(dirID)
    })
    const body = data?.data || data || {}
    const task = normalizeTask(body)
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
    const data = await guangyaRequest(user_id, '/nd.bizcloudcollection.s/v1/list_task', {
      page: 0,
      pageSize: 50,
      status: [0, 1, 2, 3, 4]
    })
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
