import { apiGuangyaMkdir } from './filecmd'
import { apiGuangyaCheckFlashUpload } from './upload'
import { apiGuangyaFileList, getGuangyaFileId, getGuangyaFileName, guangyaApiParentId, guangyaRequest, isGuangyaDir } from './dirfilelist'
import type { MiaochuanFile } from '../utils/drive-tools/miaochuan'

export interface GuangyaMiaochuanResult {
  total: number
  success: number
  skipped: number
  failed: number
  failures: { path: string; reason: string }[]
}

const bodyOf = (data: any) => data?.data || data || {}

const ensureChildDir = async (userId: string, parentId: string, name: string): Promise<string> => {
  const created = await apiGuangyaMkdir(userId, parentId, name)
  if (created.file_id) return created.file_id

  const siblings = await apiGuangyaFileList(userId, parentId, 500)
  const matched = siblings.find(item => isGuangyaDir(item) && getGuangyaFileName(item) === name)
  const fileId = matched ? getGuangyaFileId(matched) : ''
  if (!fileId) throw new Error(created.error || `创建目录失败：${name}`)
  return fileId
}

const ensureDirPath = async (userId: string, rootParentId: string, filePath: string, cache: Map<string, string>): Promise<string> => {
  const parts = filePath.split('/').filter(Boolean)
  parts.pop()
  let parentId = rootParentId
  let key = guangyaApiParentId(rootParentId) || 'root'
  for (const part of parts) {
    key = `${key}/${part}`
    if (cache.has(key)) {
      parentId = cache.get(key)!
      continue
    }
    parentId = await ensureChildDir(userId, parentId, part)
    cache.set(key, parentId)
  }
  return parentId
}

const createResCenterTask = async (userId: string, file: MiaochuanFile, parentId: string) => {
  return guangyaRequest(userId, '/nd.bizuserres.s/v1/get_res_center_token', {
    capacity: 1,
    name: file.name,
    parentId: guangyaApiParentId(parentId),
    res: file.gcid ? { fileSize: file.size } : { md5: file.md5, fileSize: file.size }
  }, 'POST', [156])
}

const deleteUploadTask = async (userId: string, taskId: string) => {
  if (!taskId) return
  await guangyaRequest(userId, '/nd.bizuserres.s/v1/file/delete_upload_task', { taskIds: [taskId] }).catch(() => undefined)
}

export const apiGuangyaImportMiaochuan = async (userId: string, parentId: string, files: MiaochuanFile[]): Promise<GuangyaMiaochuanResult> => {
  const result: GuangyaMiaochuanResult = { total: files.length, success: 0, skipped: 0, failed: 0, failures: [] }
  const dirCache = new Map<string, string>()

  for (const file of files) {
    try {
      if (!file.md5 && !file.gcid) {
        result.skipped += 1
        result.failures.push({ path: file.path, reason: '缺少 MD5/GCID' })
        continue
      }
      const targetParentId = await ensureDirPath(userId, parentId || 'guangya_root', file.path, dirCache)
      const data = await createResCenterTask(userId, file, targetParentId)
      const code = Number(data?.code)
      const body = bodyOf(data)
      const taskId = String(body?.taskId || body?.task_id || '')

      if (code === 156 || body?.fileId || body?.resId) {
        result.success += 1
        continue
      }

      if (file.gcid && taskId) {
        const flash = await apiGuangyaCheckFlashUpload(userId, taskId, file.gcid)
        if (flash.canFlashUpload) {
          result.success += 1
          continue
        }
      }

      await deleteUploadTask(userId, taskId)
      result.failed += 1
      result.failures.push({ path: file.path, reason: body?.message || data?.message || data?.msg || '秒传失败，服务端未确认可秒传' })
    } catch (error: any) {
      result.failed += 1
      result.failures.push({ path: file.path, reason: error?.message || '秒传失败' })
    }
  }

  return result
}
