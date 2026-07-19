import type { IAliGetFileModel } from '../aliapi/alimodels'
import getFileIcon from '../aliapi/fileicon'
import UserDAL from '../user/userdal'
import { humanDateTimeDateStr, humanSize } from '../utils/format'
import message from '../utils/message'
import { HanToPin } from '../utils/utils'
import { GUANGYA_API_URL, guangyaApiHeaders, refreshGuangyaAccessToken } from './auth'

export type GuangyaFileItem = Record<string, any>

const getToken = async (user_id: string) => {
  let token = UserDAL.GetUserToken(user_id)
  if (!token?.access_token) token = await UserDAL.GetUserTokenFromDB(user_id) as any
  const expireTime = new Date(token?.expire_time || 0).getTime()
  if (token?.refresh_token && (!token.access_token || (expireTime && expireTime <= Date.now()))) {
    const refreshed = await refreshGuangyaAccessToken(token)
    if (refreshed) {
      UserDAL.SaveUserToken(refreshed)
      token = refreshed
    }
  }
  return token
}

export const guangyaApiParentId = (parentId: string | number | undefined) => {
  const id = String(parentId || '')
  return id === 'guangya_root' || id === '0' || id === '/' ? '' : id
}

export const guangyaRequest = async (user_id: string, endpoint: string, body: any = {}, method: 'GET' | 'POST' = 'POST', allowedCodes: number[] = []): Promise<any> => {
  let token = await getToken(user_id)
  if (!token?.access_token) throw new Error('未登录光鸭云盘')
  const request = async (accessToken: typeof token) => {
    const resp = await fetch(`${GUANGYA_API_URL}${endpoint}`, {
      method,
      headers: guangyaApiHeaders(accessToken),
      body: method === 'POST' ? JSON.stringify(body || {}) : undefined
    })
    return { resp, data: await resp.json().catch(() => undefined) }
  }
  let { resp, data } = await request(token)
  const code = Number(data?.code)
  const authFailed = resp.status === 401 || code === 401 || /access\s*token.*(?:invalid|expired)|accessToken.*(?:无效|过期)/i.test(String(data?.message || data?.msg || data?.error || ''))
  if (authFailed && token.refresh_token) {
    const refreshed = await refreshGuangyaAccessToken(token)
    if (refreshed?.access_token) {
      UserDAL.SaveUserToken(refreshed)
      token = refreshed
      ;({ resp, data } = await request(token))
    }
  }
  const finalCode = Number(data?.code)
  if (!resp.ok || (data?.success === false && !allowedCodes.includes(finalCode)) || finalCode === 401) throw new Error(data?.message || data?.msg || `光鸭云盘请求失败 HTTP ${resp.status}`)
  return data
}

const listFromResponse = (data: any): GuangyaFileItem[] => {
  const raw = data?.data || data
  const list = raw?.list || raw?.items || raw?.files || raw?.records || raw?.content || raw?.fileList || []
  return Array.isArray(list) ? list : []
}

export const isGuangyaDir = (item: GuangyaFileItem): boolean => {
  const type = String(item.type ?? item.fileType ?? item.resType ?? item.category ?? '').toLowerCase()
  return !!(item.isDir || item.isdir || item.dir || item.isFolder || item.folder || item.dirName || type === 'folder' || type === 'dir' || type === 'directory' || type === '0')
}

export const getGuangyaFileId = (item: GuangyaFileItem): string => String(item.fileId || item.file_id || item.id || item.resId || item.resourceId || '')

export const getGuangyaFileName = (item: GuangyaFileItem): string => String(item.name || item.fileName || item.filename || item.dirName || item.title || '')

export const apiGuangyaFileList = async (user_id: string, parentId: string | number, size = 200): Promise<GuangyaFileItem[]> => {
  try {
    const all: GuangyaFileItem[] = []
    let page = 0
    let hasMore = true
    while (hasMore && all.length < size) {
      const data = await guangyaRequest(user_id, '/userres/v1/file/get_file_list', {
        parentId: guangyaApiParentId(parentId),
        page,
        pageSize: Math.min(100, size - all.length),
        orderBy: 0,
        sortType: 0
      })
      const list = listFromResponse(data)
      all.push(...list)
      hasMore = list.length > 0 && list.length >= Math.min(100, size - all.length)
      page += 1
    }
    return all
  } catch (error: any) {
    message.error(error?.message || '获取光鸭云盘文件列表失败')
    return []
  }
}

export const apiGuangyaVideoList = async (user_id: string, size = 200): Promise<GuangyaFileItem[]> => {
  try {
    const all: GuangyaFileItem[] = []
    let page = 0
    let hasMore = true
    while (hasMore && all.length < size) {
      const data = await guangyaRequest(user_id, '/userres/v1/file/get_file_list', {
        parentId: '*',
        page,
        pageSize: Math.min(100, size - all.length),
        orderBy: 3,
        sortType: 1,
        fileTypes: [2],
        resType: 1,
        needPlayRecord: false
      })
      const list = listFromResponse(data)
      all.push(...list)
      hasMore = list.length > 0 && list.length >= Math.min(100, size - all.length)
      page += 1
    }
    return all
  } catch (error: any) {
    message.error(error?.message || '获取光鸭云盘视频列表失败')
    return []
  }
}

export const apiGuangyaFileDetail = async (user_id: string, fileId: string): Promise<GuangyaFileItem | null> => {
  if (fileId === 'guangya_root' || fileId === '0' || fileId === '/' || fileId === '') return { fileId: 'guangya_root', name: '网盘文件', isDir: true }
  try {
    const data = await guangyaRequest(user_id, '/nd.bizuserres.s/v1/file/get_file_detail', { fileId })
    return data?.data || data || null
  } catch {
    const list = await apiGuangyaFileList(user_id, 'guangya_root')
    return list.find((item) => getGuangyaFileId(item) === String(fileId)) || null
  }
}

export const apiGuangyaDownloadInfo = async (user_id: string, fileId: string): Promise<{ url: string; size: number; name: string; error: string }> => {
  try {
    const data = await guangyaRequest(user_id, '/nd.bizuserres.s/v1/get_res_download_url', { fileId })
    const info = data?.data || data || {}
    const url = String(info.url || info.downloadUrl || info.download_url || info.cdnUrl || info.cdn_url || '')
    return { url, size: Number(info.size || 0), name: info.name || info.fileName || '', error: url ? '' : '光鸭云盘未返回下载链接' }
  } catch (error: any) {
    return { url: '', size: 0, name: '', error: error?.message || '获取光鸭云盘下载地址失败' }
  }
}

export const mapGuangyaFileToAliModel = (item: GuangyaFileItem, drive_id = 'guangya', parentId = 'guangya_root'): IAliGetFileModel => {
  const file_id = getGuangyaFileId(item)
  const name = getGuangyaFileName(item)
  const isDir = isGuangyaDir(item)
  const ext = isDir ? '' : (name.split('.').pop() || '')
  const size = Number(item.size || item.fileSize || item.file_size || 0)
  const timeSource = item.updatedAt || item.updateAt || item.updated_at || item.updateTime || item.createAt || item.createdAt || item.created_at || item.createTime || ''
  const time = timeSource ? new Date(timeSource).getTime() : 0
  const iconInfo = isDir ? ['', 'iconfile-folder'] : getFileIcon('', ext, ext, '', size)
  const model: any = {
    __v_skip: true,
    drive_id,
    file_id,
    parent_file_id: String(item.parentId || item.parentFileId || item.parent_id || parentId || 'guangya_root'),
    name,
    namesearch: HanToPin(name),
    ext,
    mime_type: item.mimeType || item.mime_type || '',
    mime_extension: ext,
    category: iconInfo[0],
    icon: iconInfo[1],
    file_count: 0,
    size,
    sizeStr: humanSize(size),
    time,
    timeStr: time ? humanDateTimeDateStr(new Date(time).toISOString()) : '',
    starred: false,
    isDir,
    thumbnail: item.thumbnail || item.thumbnailUrl || item.thumbnailURL || item.cover || '',
    content_hash: item.contentHash || item.content_hash || item.md5 || item.sha1 || item.gcid || '',
    description: `guangya_id:${file_id}`
  }
  return model
}
