import { googleDriveRequest } from './dirfilelist'

export type GoogleDriveChange = { fileId?: string; removed?: boolean; time?: string; file?: { id?: string; name?: string; trashed?: boolean; mimeType?: string; modifiedTime?: string } }
type GoogleChangesResponse = { changes?: GoogleDriveChange[]; nextPageToken?: string; newStartPageToken?: string }

export const buildGoogleStartPageTokenPath = (driveId = '') => {
  const params = new URLSearchParams({ supportsAllDrives: 'true' })
  if (driveId) { params.set('driveId', driveId); params.set('supportsAllDrives', 'true') }
  return `/changes/startPageToken?${params.toString()}`
}

export const buildGoogleChangesPath = (pageToken: string, driveId = '') => {
  const params = new URLSearchParams({ pageToken, pageSize: '1000', fields: 'nextPageToken,newStartPageToken,changes(fileId,removed,time,file(id,name,trashed,mimeType,modifiedTime))', spaces: 'drive', supportsAllDrives: 'true', includeItemsFromAllDrives: 'true' })
  if (driveId) { params.set('driveId', driveId); params.set('corpora', 'drive') }
  return `/changes?${params.toString()}`
}

export const apiGoogleStartPageToken = async (userId: string, driveId = '') => {
  const data = await googleDriveRequest<{ startPageToken?: string }>(userId, buildGoogleStartPageTokenPath(driveId), { method: 'GET' }, '获取 Google Drive 增量同步游标失败')
  return data?.startPageToken || ''
}

export const apiGoogleChanges = async (userId: string, startPageToken: string, driveId = '') => {
  const changes: GoogleDriveChange[] = []
  let pageToken = startPageToken
  let newStartPageToken = ''
  while (pageToken) {
    const data = await googleDriveRequest<GoogleChangesResponse>(userId, buildGoogleChangesPath(pageToken, driveId), { method: 'GET' }, '获取 Google Drive 增量变更失败')
    if (!data) break
    changes.push(...(data.changes || []))
    pageToken = data.nextPageToken || ''
    newStartPageToken = data.newStartPageToken || newStartPageToken
  }
  return { changes, newStartPageToken }
}
