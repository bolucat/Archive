import { getDropboxToken } from './dirfilelist'

const DROPBOX_CONTENT_API_HOST = 'https://content.dropboxapi.com/2'

export const buildDropboxZipDownloadHeaders = (accessToken: string, folderId: string): Record<string, string> => ({
  Authorization: `Bearer ${accessToken}`,
  'Dropbox-API-Arg': JSON.stringify({ path: folderId })
})

export const apiDropboxZipDownload = async (user_id: string, folderId: string): Promise<{ url: string; headers: Record<string, string>; error: string }> => {
  const token = await getDropboxToken(user_id)
  if (!token?.access_token) return { url: '', headers: {}, error: '未登录 Dropbox' }
  return {
    url: `${DROPBOX_CONTENT_API_HOST}/files/download_zip`,
    headers: buildDropboxZipDownloadHeaders(token.access_token, folderId),
    error: ''
  }
}
