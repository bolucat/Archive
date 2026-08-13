import { guangyaApiParentId, guangyaRequest } from './dirfilelist'
import { MD5, enc, lib } from 'crypto-js'
import { Sleep } from '../utils/format'

export interface GuangyaUploadTokenData {
  taskId: string
  objectPath?: string
  params?: {
    url?: string
    multipart?: Record<string, unknown>
  }
}

const getBody = (data: any) => data?.data || data || {}

export const apiGuangyaUploadTaskFile = async (tokenData: GuangyaUploadTokenData, name: string, file: Blob): Promise<string> => {
  const url = String(tokenData.params?.url || '')
  const multipart = tokenData.params?.multipart || {}
  const key = String(multipart.key || tokenData.objectPath || '')
  const accessKeyId = String(multipart.OSSAccessKeyId || multipart.ossAccessKeyId || '')
  const policy = String(multipart.policy || '')
  const signature = String(multipart.Signature || multipart.signature || '')
  if (!url || !key || !accessKeyId || !policy || !signature) return '光鸭云盘未返回表单上传凭证'

  const form = new FormData()
  form.set('key', key)
  form.set('OSSAccessKeyId', accessKeyId)
  form.set('policy', policy)
  form.set('Signature', signature)
  for (const [field, value] of Object.entries(multipart)) {
    if (['key', 'OSSAccessKeyId', 'ossAccessKeyId', 'policy', 'Signature', 'signature'].includes(field) || value === undefined || value === null || value === '') continue
    form.set(field, String(value))
  }
  form.set('file', file, name)
  const resp = await fetch(url, { method: 'POST', body: form })
  return resp.ok ? '' : '光鸭云盘上传文件内容失败'
}

export const apiGuangyaUploadToken = async (user_id: string, name: string, fileSize: number, parentId: string, md5?: string): Promise<{ data?: GuangyaUploadTokenData; error: string }> => {
  try {
    const body: any = {
      capacity: 1,
      name,
      parentId: guangyaApiParentId(parentId),
      res: { fileSize }
    }
    if (md5) body.res.md5 = md5
    const data = await guangyaRequest(user_id, '/nd.bizuserres.s/v1/get_res_center_token', body)
    const tokenData = getBody(data)
    if (!tokenData?.taskId) return { error: '光鸭云盘未返回上传 taskId' }
    return { data: tokenData as GuangyaUploadTokenData, error: '' }
  } catch (error: any) {
    return { error: error?.message || '获取光鸭云盘上传凭证失败' }
  }
}

export const apiGuangyaCheckFlashUpload = async (user_id: string, taskId: string, gcid: string): Promise<{ canFlashUpload: boolean; error: string }> => {
  try {
    const data = await guangyaRequest(user_id, '/nd.bizuserres.s/v1/check_can_flash_upload', { taskId, gcid })
    const body = getBody(data)
    return { canFlashUpload: !!body?.canFlashUpload, error: '' }
  } catch (error: any) {
    return { canFlashUpload: false, error: error?.message || '光鸭云盘秒传检测失败' }
  }
}

export const apiGuangyaUploadInfo = async (user_id: string, taskId: string): Promise<{ fileId: string; uploading: boolean; error: string; raw?: any }> => {
  try {
    const data = await guangyaRequest(user_id, '/nd.bizuserres.s/v1/file/get_info_by_task_id', { taskId })
    const body = getBody(data)
    const fileId = String(body?.fileId || body?.id || body?.file_id || body?.resId || '')
    const msg = String(data?.msg || data?.message || body?.msg || body?.message || '')
    const status = String(body?.status ?? body?.uploadStatus ?? body?.state ?? '').toLowerCase()
    const uploading = /文件上传中|上传中|处理中|uploading|processing|pending/i.test(msg) || ['0', '1', 'uploading', 'processing', 'pending'].includes(status)
    return { fileId, uploading, error: fileId || uploading ? '' : (msg || '光鸭云盘未返回上传文件 ID'), raw: body }
  } catch (error: any) {
    const message = error?.message || '获取光鸭云盘上传结果失败'
    return { fileId: '', uploading: message.includes('文件上传中'), error: message }
  }
}

export const apiGuangyaUploadBuffer = async (user_id: string, parentId: string, name: string, buff: Buffer): Promise<{ file_id: string; error: string }> => {
  const md5 = MD5(lib.WordArray.create(buff as any)).toString(enc.Base64)
  const tokenResp = await apiGuangyaUploadToken(user_id, name, buff.length, parentId, md5)
  if (!tokenResp.data) return { file_id: '', error: tokenResp.error || '创建光鸭云盘文件失败' }
  let info = await apiGuangyaUploadInfo(user_id, tokenResp.data.taskId)
  if (info.fileId) return { file_id: info.fileId, error: '' }
  const uploadError = await apiGuangyaUploadTaskFile(tokenResp.data, name, new Blob([new Uint8Array(buff)], { type: 'application/octet-stream' }))
  if (uploadError) return { file_id: '', error: uploadError }
  for (let i = 0; i < 10; i++) {
    info = await apiGuangyaUploadInfo(user_id, tokenResp.data.taskId)
    if (info.fileId) return { file_id: info.fileId, error: '' }
    if (!info.uploading && info.error) return { file_id: '', error: info.error }
    await Sleep(1000)
  }
  return { file_id: '', error: '光鸭云盘上传完成确认超时' }
}
