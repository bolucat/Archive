import { googleDriveRequest } from './dirfilelist'

export type GoogleDownloadOperation = { name?: string; done?: boolean; error?: { message?: string }; response?: Record<string, unknown> }

export type GoogleDownloadResponse = { downloadUri?: string; partialDownloadAllowed?: boolean }

export const buildGoogleDownloadPath = (fileId: string, mimeType = '', revisionId = '', resourceKey = '') => {
  const params = new URLSearchParams({ supportsAllDrives: 'true' })
  if (mimeType) params.set('mimeType', mimeType)
  if (revisionId) params.set('revisionId', revisionId)
  if (resourceKey) params.set('resourceKey', resourceKey)
  return `/files/${encodeURIComponent(fileId)}/download?${params.toString()}`
}
export const buildGoogleOperationPath = (name: string, resourceKey = '') => {
  const path = name.startsWith('operations/') ? `/${name}` : `/operations/${encodeURIComponent(name)}`
  return resourceKey ? `${path}?${new URLSearchParams({ resourceKey }).toString()}` : path
}
export const apiGoogleStartDownload = async (userId: string, fileId: string, mimeType = '', revisionId = '', resourceKey = '') => await googleDriveRequest<GoogleDownloadOperation>(userId, buildGoogleDownloadPath(fileId, mimeType, revisionId, resourceKey), { method: 'POST' }, '创建 Google Drive 下载操作失败')
export const apiGoogleGetDownloadOperation = async (userId: string, name: string, resourceKey = '') => await googleDriveRequest<GoogleDownloadOperation>(userId, buildGoogleOperationPath(name, resourceKey), { method: 'GET' }, '查询 Google Drive 下载操作失败')

const getDownloadUri = (operation: GoogleDownloadOperation | null) => String((operation?.response as GoogleDownloadResponse | undefined)?.downloadUri || '')

export const apiGoogleResolveDownload = async (userId: string, fileId: string, mimeType = '', revisionId = '', resourceKey = '') => {
  let operation = await apiGoogleStartDownload(userId, fileId, mimeType, revisionId, resourceKey)
  if (!operation) return ''
  if (operation.done) return getDownloadUri(operation)
  if (!operation.name) return ''
  const operationName = operation.name

  // Google Drive recommends polling an LRO with exponential backoff (starting at 10 seconds).
  for (const delay of [10_000, 20_000, 40_000]) {
    await new Promise(resolve => setTimeout(resolve, delay))
    operation = await apiGoogleGetDownloadOperation(userId, operationName, resourceKey)
    if (!operation) return ''
    if (operation.done) return getDownloadUri(operation)
  }
  return ''
}
