import { dropboxRpc } from './dirfilelist'

type DropboxSaveUrlLaunch = { async_job_id?: string }
type DropboxSaveUrlStatus = { '.tag'?: 'in_progress' | 'complete' | 'failed'; error_summary?: string }

export const buildDropboxSaveUrlBody = (path: string, url: string) => ({ path, url })

export const apiDropboxOfflineCreate = async (user_id: string, url: string, targetPath: string): Promise<{ taskId: string; error: string }> => {
  const data = await dropboxRpc<DropboxSaveUrlLaunch>(user_id, '/files/save_url', buildDropboxSaveUrlBody(targetPath, url), '创建 Dropbox 离线下载失败')
  return { taskId: data?.async_job_id || '', error: data?.async_job_id ? '' : '创建 Dropbox 离线下载失败' }
}

export const apiDropboxOfflineProcess = async (user_id: string, taskId: string): Promise<{ complete: boolean; failed: boolean; error: string }> => {
  const data = await dropboxRpc<DropboxSaveUrlStatus>(user_id, '/files/save_url/check_job_status', { async_job_id: taskId }, '查询 Dropbox 离线下载失败')
  if (!data) return { complete: false, failed: true, error: '查询 Dropbox 离线下载失败' }
  return { complete: data['.tag'] === 'complete', failed: data['.tag'] === 'failed', error: data.error_summary || '' }
}
