import { guangyaApiParentId, guangyaRequest } from './dirfilelist'
import { HmacSHA1, MD5, enc, lib } from 'crypto-js'
import { Sleep } from '../utils/format'

export interface GuangyaUploadTokenData {
  taskId: string
  creds?: {
    accessKeyID?: string
    accessKeyId?: string
    accessKeySecret?: string
    secretAccessKey?: string
    securityToken?: string
    sessionToken?: string
  }
  fullEndPoint?: string
  bucketName?: string
  objectPath?: string
}

const getBody = (data: any) => data?.data || data || {}

const getOssCredentials = (tokenData: GuangyaUploadTokenData) => {
  const creds = tokenData.creds || {}
  return {
    accessKeyID: creds.accessKeyID || creds.accessKeyId || '',
    secretAccessKey: creds.secretAccessKey || creds.accessKeySecret || '',
    sessionToken: creds.sessionToken || creds.securityToken || ''
  }
}

const normalizeEndpoint = (endpoint: string) => endpoint.startsWith('http') ? endpoint.replace(/\/$/, '') : `https://${endpoint.replace(/\/$/, '')}`

const signOss = (method: string, objectPath: string, query: string, date: string, contentType: string, tokenData: GuangyaUploadTokenData) => {
  const { accessKeyID, secretAccessKey, sessionToken } = getOssCredentials(tokenData)
  const bucketName = tokenData.bucketName || ''
  const ossHeaders = sessionToken ? `x-oss-security-token:${sessionToken}\n` : ''
  const resource = `/${bucketName}/${objectPath}${query ? `?${query}` : ''}`
  const canonical = `${method}\n\n${contentType}\n${date}\n${ossHeaders}${resource}`
  const signature = HmacSHA1(canonical, secretAccessKey).toString(enc.Base64)
  return `OSS ${accessKeyID}:${signature}`
}

const apiGuangyaPutObject = async (tokenData: GuangyaUploadTokenData, buff: Buffer): Promise<string> => {
  if (!tokenData.fullEndPoint || !tokenData.bucketName || !tokenData.objectPath) return '光鸭云盘上传凭证不完整'
  const endpoint = normalizeEndpoint(tokenData.fullEndPoint)
  const objectPath = tokenData.objectPath
  const date = new Date().toUTCString()
  const contentType = 'application/octet-stream'
  const { sessionToken } = getOssCredentials(tokenData)
  const headers: Record<string, string> = {
    Date: date,
    'Content-Type': contentType,
    Authorization: signOss('PUT', objectPath, '', date, contentType, tokenData)
  }
  if (sessionToken) headers['x-oss-security-token'] = sessionToken
  const resp = await fetch(`${endpoint}/${objectPath}`, { method: 'PUT', headers, body: buff as any })
  return resp.ok ? '' : '光鸭云盘上传文件内容失败'
}

export const apiGuangyaUploadToken = async (user_id: string, name: string, fileSize: number, parentId: string, md5?: string): Promise<{ data?: GuangyaUploadTokenData; error: string }> => {
  try {
    const body: any = {
      capacity: 2,
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
    return { fileId, uploading: msg.includes('文件上传中'), error: fileId || msg.includes('文件上传中') ? '' : (msg || '光鸭云盘未返回上传文件 ID'), raw: body }
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
  const ossError = await apiGuangyaPutObject(tokenResp.data, buff)
  if (ossError) return { file_id: '', error: ossError }
  for (let i = 0; i < 3; i++) {
    info = await apiGuangyaUploadInfo(user_id, tokenResp.data.taskId)
    if (info.fileId) return { file_id: info.fileId, error: '' }
    if (!info.uploading && info.error) return { file_id: '', error: info.error }
    await Sleep(1500)
  }
  return { file_id: '', error: '光鸭云盘上传完成确认超时' }
}
