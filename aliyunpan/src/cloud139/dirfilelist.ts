import type { IAliGetFileModel } from '../aliapi/alimodels'
import getFileIcon from '../aliapi/fileicon'
import UserDAL from '../user/userdal'
import { humanDateTimeDateStr, humanSize } from '../utils/format'
import message from '../utils/message'
import { HanToPin } from '../utils/utils'
import { CLOUD139_ROUTE_POLICY_URL, CLOUD139_USER_AGENT, cloud139Headers } from './auth'

export type Cloud139FileItem = {
  fileId?: string
  catalogId?: string
  parentFileId?: string
  parentCatalogId?: string
  name?: string
  catalogName?: string
  type?: string
  size?: number | string
  updated_at?: string
  updatedAt?: string
  updateTime?: string
  created_at?: string
  createTime?: string
  thumbnailURL?: string
  thumbnailUrls?: { url?: string }[]
}

const hostCache = new Map<string, string>()

const getToken = async (user_id: string) => {
  let token = UserDAL.GetUserToken(user_id)
  if (!token?.access_token) token = await UserDAL.GetUserTokenFromDB(user_id) as any
  return token
}

const apiParentId = (parentId: string | number) => {
  const id = String(parentId || '/')
  return id === 'cloud139_root' || id === '0' || id === '' ? '/' : id
}

const cloud139Request = async (user_id: string, endpoint: string, body: any): Promise<any> => {
  const token = await getToken(user_id)
  if (!token?.access_token) throw new Error('未登录 139 云盘')
  const postBody = JSON.stringify(body || {})
  const url = `${await getCloud139Host(user_id)}/orchestration${endpoint}`
  const resp = await fetch(url, {
    method: 'POST',
    headers: cloud139Headers(token.access_token, postBody),
    body: postBody
  })
  const data = await resp.json().catch(() => undefined)
  if (!resp.ok || data?.success === false || data?.code === '9000' || data?.code === '9008' || data?.code === '9100') {
    throw new Error(data?.message || data?.msg || `139 云盘请求失败 HTTP ${resp.status}`)
  }
  return data
}

const getCloud139Host = async (user_id: string): Promise<string> => {
  const cached = hostCache.get(user_id)
  if (cached) return cached
  const token = await getToken(user_id)
  if (!token?.access_token) throw new Error('未登录 139 云盘')
  const account = (token.user_name || token.nick_name || token.user_id).replace(/^cloud139_/, '').replace(/^139云盘\s*/, '')
  const body = JSON.stringify({
    userInfo: { userType: 1, accountType: 1, accountName: account },
    modAddrType: 1
  })
  const routeResp = await fetch(CLOUD139_ROUTE_POLICY_URL, {
    method: 'POST',
    headers: cloud139Headers(token.access_token, body, true),
    body
  })
  const data: any = await routeResp.json().catch(() => undefined)
  const routeList: any[] = data?.data?.routePolicyList || data?.routePolicyList || []
  const personal = Array.isArray(routeList) ? routeList.find((item) => item?.modName === 'personal') || routeList[0] : null
  const host: string = personal?.httpsUrl || personal?.httpUrl || 'https://ose.caiyun.feixin.10086.cn'
  hostCache.set(user_id, host)
  return host
}

const listFromResponse = (data: any): Cloud139FileItem[] => {
  const raw = data?.data || data
  const list = raw?.list || raw?.content || raw?.files || raw?.fileList || []
  return Array.isArray(list) ? list : []
}

export const apiCloud139FileList = async (user_id: string, parentId: string | number, size = 100): Promise<Cloud139FileItem[]> => {
  try {
    const data = await cloud139Request(user_id, '/file/list', {
      imageThumbnailStyleList: ['Small', 'Large'],
      orderBy: 'updated_at',
      orderDirection: 'DESC',
      pageInfo: { pageCursor: '', pageSize: size },
      parentFileId: apiParentId(parentId)
    })
    return listFromResponse(data)
  } catch (error: any) {
    message.error(error?.message || '获取 139 云盘文件列表失败')
    return []
  }
}

export const apiCloud139FileDetail = async (user_id: string, fileId: string): Promise<Cloud139FileItem | null> => {
  if (fileId === 'cloud139_root' || fileId === '/' || fileId === '0') return { fileId: '/', name: '网盘文件', type: 'folder' }
  const list = await apiCloud139FileList(user_id, '/')
  return list.find((item) => String(item.fileId || item.catalogId) === String(fileId)) || null
}

export const apiCloud139DownloadInfo = async (user_id: string, fileId: string): Promise<{ url: string; size: number; name: string; error: string }> => {
  try {
    const data = await cloud139Request(user_id, '/file/getDownloadUrl', { fileId })
    const info = data?.data || data || {}
    const url = info.cdnUrl || info.url || info.downloadUrl || ''
    return { url, size: Number(info.size || 0), name: info.fileName || info.name || '', error: url ? '' : '139 云盘未返回下载链接' }
  } catch (error: any) {
    return { url: '', size: 0, name: '', error: error?.message || '获取 139 云盘下载地址失败' }
  }
}

export const cloud139DownloadHeaders = () => ({
  'User-Agent': CLOUD139_USER_AGENT,
  Referer: 'https://yun.139.com/',
  Origin: 'https://yun.139.com'
})

export const mapCloud139FileToAliModel = (item: Cloud139FileItem, drive_id = 'cloud139', parentId = 'cloud139_root'): IAliGetFileModel => {
  const file_id = String(item.fileId || item.catalogId || '')
  const name = String(item.name || item.catalogName || '')
  const isDir = item.type === 'folder'
  const ext = isDir ? '' : (name.split('.').pop() || '')
  const size = Number(item.size || 0)
  const timeSource = item.updated_at || item.updatedAt || item.updateTime || item.created_at || item.createTime || ''
  const time = timeSource ? new Date(timeSource).getTime() : 0
  const iconInfo = isDir ? ['', 'iconfile-folder'] : getFileIcon('', ext, ext, '', size)
  return {
    __v_skip: true,
    drive_id,
    file_id,
    parent_file_id: String(item.parentFileId || item.parentCatalogId || parentId || 'cloud139_root'),
    name,
    namesearch: HanToPin(name),
    ext,
    mime_type: '',
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
    thumbnail: item.thumbnailUrls?.find((thumb) => thumb?.url)?.url || item.thumbnailURL || '',
    description: `cloud139_id:${file_id}`
  }
}

export { cloud139Request, apiParentId as cloud139ApiParentId }
