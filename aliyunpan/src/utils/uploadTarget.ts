import { resolveDriveProvider } from './driveProvider'

/** Keep Aliyun's legacy ID guard without rejecting other providers' folder IDs. */
export const isValidDropUploadTarget = (userId: string, driveId: string, fileId: string): boolean => {
  const id = String(fileId || '')
  const route = resolveDriveProvider(userId, driveId)
  if (!route.isValid) return id.length === 40 || id.includes('root')
  if (route.provider !== 'aliyun') return Boolean(id)
  return id.length === 40 || id.includes('root')
}
