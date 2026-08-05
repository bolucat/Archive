import { boxApiRequest, getBoxToken } from './dirfilelist'

type BoxZipItem = { id: string; type: 'file' | 'folder' }
type BoxZipResponse = { download_url?: string; status_url?: string; expires_at?: string }
type BoxZipStatus = { state?: string; download_url?: string; message?: string }

export const buildBoxZipBody = (items: BoxZipItem[], downloadFileName: string) => ({ items, download_file_name: downloadFileName })

export const apiBoxCreateZip = async (user_id: string, items: BoxZipItem[], downloadFileName: string): Promise<{ data?: BoxZipResponse; error: string }> => {
  const data = await boxApiRequest<BoxZipResponse>(user_id, '/zip_downloads', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(buildBoxZipBody(items, downloadFileName)) }, '创建 Box ZIP 下载失败')
  return data?.download_url && data.status_url ? { data, error: '' } : { error: 'Box 未返回 ZIP 下载地址' }
}

export const apiBoxWaitZip = async (user_id: string, zip: BoxZipResponse, attempts = 120): Promise<{ url?: string; headers?: Record<string, string>; error: string }> => {
  if (!zip.status_url || !zip.download_url) return { error: 'Box ZIP 下载状态地址不完整' }
  for (let attempt = 0; attempt < attempts; attempt++) {
    const status = await boxApiRequest<BoxZipStatus>(user_id, zip.status_url, { method: 'GET' }, '查询 Box ZIP 下载状态失败')
    if (status?.state === 'succeeded') {
      const token = await getBoxToken(user_id)
      return { url: status.download_url || zip.download_url, headers: token?.access_token ? { Authorization: `Bearer ${token.access_token}` } : {}, error: '' }
    }
    if (status?.state === 'failed') return { error: status.message || 'Box ZIP 创建失败' }
    await new Promise(resolve => setTimeout(resolve, 1000))
  }
  return { error: 'Box ZIP 创建超时，请稍后重试' }
}
