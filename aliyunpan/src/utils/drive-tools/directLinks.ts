import type { IAliGetFileModel } from '../../aliapi/alimodels'
import { resolveDriveProvider } from '../driveProvider'
import type { IDownloadUrl } from '../../aliapi/models'
import { resolveDriveFileToken } from '../../drive/account'
import UserDAL from '../../user/userdal'

export type DirectLinkFormat = 'url' | 'aria2'

export interface DirectLinkItem {
  name: string
  fileId: string
  driveId: string
  userId: string
  url: string
  size: number
  headers?: Record<string, string>
}

export interface DirectLinkExportResult {
  total: number
  success: number
  failed: number
  links: DirectLinkItem[]
  failures: { name: string; reason: string }[]
  text: string
}

const MAX_DIRECTORY_LIST_ITEMS = 20_000

export const normalizeDriveToolDriveId = (driveId: string): string => {
  const id = String(driveId || '')
  const aliases: Record<string, string> = {
    '123': 'cloud123',
    '115': 'drive115',
    '139': 'cloud139',
    '189': 'cloud189'
  }
  return aliases[id] || id
}

export const normalizeDriveToolPlatform = (platform: string): string => {
  const value = String(platform || '')
  const aliases: Record<string, string> = {
    '123': 'cloud123',
    drive115: '115',
    cloud139: '139',
    cloud189: '189'
  }
  return aliases[value] || value
}

export const driveToolPlatformMatches = (tokenfrom: string, requested?: string): boolean => {
  if (!requested) return true
  return normalizeDriveToolPlatform(tokenfrom || 'aliyun') === normalizeDriveToolPlatform(requested)
}

export const driveToolRootIdFor = (driveId: string): string => {
  const map: Record<string, string> = {
    cloud123: 'cloud_root',
    drive115: 'drive115_root',
    baidu: 'baidu_root',
    pikpak: 'pikpak_root',
    quark: 'quark_root',
    cloud139: 'cloud139_root',
    cloud189: 'cloud189_root',
    guangya: 'guangya_root',
    dropbox: 'dropbox_root',
    onedrive: 'onedrive_root',
    box: 'box_root'
  }
  return map[normalizeDriveToolDriveId(driveId)] || 'root'
}

export const driveToolDriveIdForPlatform = (platform: string, defaultDriveId = ''): string => {
  if (platform === 'aliyun') return defaultDriveId
  return normalizeDriveToolDriveId(defaultDriveId || platform)
}

const listAllCloud123Children = async (userId: string, parentId: string | number, driveId: string): Promise<IAliGetFileModel[]> => {
  const { apiCloud123FileListPage, mapCloud123FileToAliModel } = await import('../../cloud123/dirfilelist')
  const output: IAliGetFileModel[] = []
  const limit = 100
  let lastFileId: string | number = ''
  for (let i = 0; i < 200; i++) {
    const page = await apiCloud123FileListPage(userId, parentId, limit, false, '', 0, lastFileId, true)
    output.push(...page.items.map(item => ({ ...mapCloud123FileToAliModel(item), drive_id: driveId })))
    if (!page.items.length || page.items.length < limit || page.lastFileId === -1 || page.lastFileId === Number(lastFileId)) break
    lastFileId = page.lastFileId
  }
  return output
}

const listAllDrive115Children = async (userId: string, parentId: string | number, driveId: string): Promise<IAliGetFileModel[]> => {
  const { apiDrive115FileList, mapDrive115FileToAliModel } = await import('../../cloud115/dirfilelist')
  const output: IAliGetFileModel[] = []
  const limit = 200
  for (let offset = 0; offset < MAX_DIRECTORY_LIST_ITEMS; offset += limit) {
    const page = await apiDrive115FileList(userId, Number(parentId || 0), limit, offset, true, { silent: true, strict: true })
    output.push(...page.map(item => mapDrive115FileToAliModel(item, driveId)))
    if (page.length < limit) break
  }
  return output
}

const listAllBaiduChildren = async (userId: string, parentId: string | number, driveId: string): Promise<IAliGetFileModel[]> => {
  const [{ apiBaiduFileList, mapBaiduFileToAliModel }, { resolveBaiduDirectoryPath }] = await Promise.all([
    import('../../cloudbaidu/dirfilelist'),
    import('../../cloudbaidu/adapter')
  ])
  const parentPath = await resolveBaiduDirectoryPath(userId, driveId, String(parentId))
  if (!parentPath) throw new Error('无法确定百度网盘目录路径，请刷新目录后重试')
  const output: IAliGetFileModel[] = []
  const limit = 1000
  for (let start = 0; output.length < MAX_DIRECTORY_LIST_ITEMS; start += limit) {
    const page = await apiBaiduFileList(userId, parentPath, 'name', start, limit, 0, true)
    output.push(...page.map(item => mapBaiduFileToAliModel(item, driveId, parentPath)))
    if (page.length < limit) break
  }
  return output
}

const listAllPikPakChildren = async (userId: string, parentId: string, driveId: string): Promise<IAliGetFileModel[]> => {
  const { apiPikPakFileList, mapPikPakFileToAliModel } = await import('../../pikpak/dirfilelist')
  const output: IAliGetFileModel[] = []
  const seen = new Set<string>()
  let pageToken = ''
  do {
    const page = await apiPikPakFileList(userId, parentId, 100, pageToken, false, true)
    output.push(...page.items.map(item => mapPikPakFileToAliModel(item, driveId, parentId)))
    pageToken = page.nextPageToken
    if (pageToken && seen.has(pageToken)) break
    seen.add(pageToken)
  } while (pageToken && output.length < MAX_DIRECTORY_LIST_ITEMS)
  return output
}

const listAllQuarkChildren = async (userId: string, parentId: string | number, driveId: string, fileId: string): Promise<IAliGetFileModel[]> => {
  const { apiQuarkFileList, mapQuarkFileToAliModel } = await import('../../quark/dirfilelist')
  const output: IAliGetFileModel[] = []
  const size = 200
  for (let pageNumber = 1; output.length < MAX_DIRECTORY_LIST_ITEMS; pageNumber++) {
    const page = await apiQuarkFileList(userId, String(parentId), size, pageNumber, true)
    output.push(...page.items.map(item => mapQuarkFileToAliModel(item, driveId, fileId)))
    if (!page.items.length || output.length >= page.total || page.items.length < size) break
  }
  return output
}

const listAllCloud139Children = async (userId: string, parentId: string | number, driveId: string, fileId: string): Promise<IAliGetFileModel[]> => {
  const { apiCloud139FileListPage, mapCloud139FileToAliModel } = await import('../../cloud139/dirfilelist')
  const output: IAliGetFileModel[] = []
  const seen = new Set<string>()
  let cursor = ''
  do {
    const page = await apiCloud139FileListPage(userId, parentId, 200, cursor, true)
    output.push(...page.items.map(item => mapCloud139FileToAliModel(item, driveId, fileId)))
    cursor = page.nextCursor
    if (cursor && seen.has(cursor)) break
    seen.add(cursor)
  } while (cursor && output.length < MAX_DIRECTORY_LIST_ITEMS)
  return output
}

const listAllCloud189Children = async (userId: string, parentId: string | number, driveId: string, fileId: string): Promise<IAliGetFileModel[]> => {
  const { apiCloud189FileList, mapCloud189FileToAliModel } = await import('../../cloud189/dirfilelist')
  const output: IAliGetFileModel[] = []
  const size = 1000
  for (let pageNumber = 1; output.length < MAX_DIRECTORY_LIST_ITEMS; pageNumber++) {
    const page = await apiCloud189FileList(userId, parentId, size, pageNumber, true)
    output.push(...page.map(item => mapCloud189FileToAliModel(item, driveId, fileId)))
    if (page.length < size) break
  }
  return output
}

const listAllGuangyaChildren = async (userId: string, parentId: string | number, driveId: string, fileId: string): Promise<IAliGetFileModel[]> => {
  const { apiGuangyaFileListPage, mapGuangyaFileToAliModel } = await import('../../guangya/dirfilelist')
  const output: IAliGetFileModel[] = []
  for (let pageNumber = 0; output.length < MAX_DIRECTORY_LIST_ITEMS; pageNumber++) {
    const page = await apiGuangyaFileListPage(userId, parentId, pageNumber, 100)
    output.push(...page.items.map(item => mapGuangyaFileToAliModel(item, driveId, fileId)))
    if (!page.hasMore) break
  }
  return output
}

const listAllAliyunChildren = async (userId: string, driveId: string, fileId: string): Promise<IAliGetFileModel[]> => {
  const { default: AliDirFileList } = await import('../../aliapi/dirfilelist')
  const output: IAliGetFileModel[] = []
  for await (const page of AliDirFileList.ApiDirFileListPages(userId, driveId, fileId, '', 'name asc')) {
    output.push(...page)
    if (output.length >= MAX_DIRECTORY_LIST_ITEMS) break
  }
  return output
}

const providerRootParent = (driveId: string, fileId: string) => {
  const providerDriveId = normalizeDriveToolDriveId(driveId)
  if (providerDriveId === 'cloud123') return fileId === 'cloud_root' ? '0' : fileId
  if (providerDriveId === 'drive115') return fileId === 'drive115_root' ? '0' : fileId
  if (providerDriveId === 'baidu') return fileId === 'baidu_root' ? '/' : fileId
  if (providerDriveId === 'pikpak') return fileId === 'pikpak_root' ? 'pikpak_root' : fileId
  if (providerDriveId === 'quark') return fileId === 'quark_root' ? '0' : fileId
  if (providerDriveId === 'cloud139') return fileId === 'cloud139_root' ? '/' : fileId
  if (providerDriveId === 'cloud189') return fileId === 'cloud189_root' ? '-11' : fileId
  if (providerDriveId === 'guangya') return fileId === 'guangya_root' ? 'guangya_root' : fileId
  if (providerDriveId === 'dropbox') return fileId === 'dropbox_root' ? 'dropbox_root' : fileId
  if (providerDriveId === 'onedrive') return fileId === 'onedrive_root' ? 'onedrive_root' : fileId
  if (providerDriveId === 'box') return fileId === 'box_root' ? 'box_root' : fileId
  return fileId
}

export const listDriveToolChildren = async (userId: string, driveId: string, fileId: string): Promise<IAliGetFileModel[]> => {
  if (driveId.startsWith('webdav:')) {
    const { getWebDavConnection, getWebDavConnectionId, listWebDavDirectory } = await import('../webdavClient')
    const connection = getWebDavConnection(getWebDavConnectionId(driveId))
    if (!connection) throw new Error('WebDAV 连接不存在或已失效')
    return listWebDavDirectory(connection, fileId || '/')
  }
  const token = UserDAL.GetUserToken(userId) || await UserDAL.GetUserTokenFromDB(userId)
  const route = resolveDriveProvider(userId, driveId, token?.tokenfrom)
  if (!route.isValid) throw new Error(route.error)
  if (!token?.access_token) throw new Error(`未登录 ${route.provider === 'aliyun' ? '阿里云盘' : route.provider}`)
  const providerDriveId = normalizeDriveToolDriveId(driveId)
  const parentId = providerRootParent(providerDriveId, fileId)
  if (providerDriveId === 'cloud123') {
    return listAllCloud123Children(userId, parentId, providerDriveId)
  }
  if (providerDriveId === 'drive115') {
    return listAllDrive115Children(userId, parentId, providerDriveId)
  }
  if (providerDriveId === 'baidu') {
    return listAllBaiduChildren(userId, parentId, providerDriveId)
  }
  if (providerDriveId === 'pikpak') {
    return listAllPikPakChildren(userId, parentId, providerDriveId)
  }
  if (providerDriveId === 'quark') {
    return listAllQuarkChildren(userId, parentId, providerDriveId, fileId)
  }
  if (providerDriveId === 'cloud139') {
    return listAllCloud139Children(userId, parentId, providerDriveId, fileId)
  }
  if (providerDriveId === 'cloud189') {
    return listAllCloud189Children(userId, parentId, providerDriveId, fileId)
  }
  if (providerDriveId === 'guangya') {
    return listAllGuangyaChildren(userId, parentId, providerDriveId, fileId)
  }
  if (providerDriveId === 'dropbox') {
    const { apiDropboxFileList, mapDropboxFileToAliModel } = await import('../../dropbox/dirfilelist')
    return (await apiDropboxFileList(userId, parentId, 500, true)).map(item => mapDropboxFileToAliModel(item, providerDriveId, parentId))
  }
  if (providerDriveId === 'onedrive') {
    const { apiOneDriveFileList, mapOneDriveItemToAliModel } = await import('../../onedrive/dirfilelist')
    return (await apiOneDriveFileList(userId, parentId, true)).map(item => mapOneDriveItemToAliModel(item, providerDriveId, parentId))
  }
  if (providerDriveId === 'box') {
    const { apiBoxFileList, mapBoxItemToAliModel } = await import('../../box/dirfilelist')
    return (await apiBoxFileList(userId, parentId, 500, true)).map(item => mapBoxItemToAliModel(item, providerDriveId, parentId))
  }
  if (providerDriveId === 'google') {
    const { listGoogleItems } = await import('../../google/adapter')
    return (await listGoogleItems(userId, driveId, fileId, true)).items
  }
  if (route.provider !== 'aliyun') throw new Error(`当前网盘 ${route.provider} 未接入目录读取适配器`)
  return listAllAliyunChildren(userId, driveId, fileId)
}

export const flattenDriveToolFiles = async (files: IAliGetFileModel[], userId: string, maxFiles = 300): Promise<IAliGetFileModel[]> => {
  const output: IAliGetFileModel[] = []
  const queue = [...files]
  while (queue.length && output.length < maxFiles) {
    const item = queue.shift()!
    if (!item.isDir) {
      output.push(item)
      continue
    }
    const children = await listDriveToolChildren(userId, item.drive_id, item.file_id)
    queue.push(...children)
  }
  return output.slice(0, maxFiles)
}

export const formatDirectLinks = (links: DirectLinkItem[], format: DirectLinkFormat): string => {
  if (format === 'aria2') {
    return links.map(item => {
      const headers = Object.entries(item.headers || {}).map(([key, value]) => `  header=${key}: ${value}`)
      return [item.url, `  out=${item.name}`, ...headers].join('\n')
    }).join('\n\n')
  }
  return links.map(item => item.url).join('\n')
}

export const exportDirectLinks = async (files: IAliGetFileModel[], userId: string, format: DirectLinkFormat = 'url', maxFiles = 300): Promise<DirectLinkExportResult> => {
  const result: DirectLinkExportResult = { total: 0, success: 0, failed: 0, links: [], failures: [], text: '' }
  const { default: DriveFile } = await import('../../drive/file')
  const groups = new Map<string, IAliGetFileModel[]>()
  for (const file of files) {
    const token = await resolveDriveFileToken(file as IAliGetFileModel & { user_id?: string }, (file as any).user_id || (file as any).userId || userId)
    if (!token?.user_id) {
      result.total += 1
      result.failed += 1
      result.failures.push({ name: file.name, reason: `找不到 ${file.drive_id} 对应的已登录账号` })
      continue
    }
    groups.set(token.user_id, [...(groups.get(token.user_id) || []), file])
  }
  for (const [currentUserId, group] of groups) {
    const flatFiles = await flattenDriveToolFiles(group, currentUserId, maxFiles)
    result.total += flatFiles.length
    for (const file of flatFiles) {
      const token = await resolveDriveFileToken(file as IAliGetFileModel & { user_id?: string }, currentUserId)
      if (!token?.user_id) {
        result.failed += 1
        result.failures.push({ name: file.name, reason: `找不到 ${file.drive_id} 对应的已登录账号` })
        continue
      }
      const data = await DriveFile.ApiFileDownloadUrl(token.user_id, file.drive_id, file.file_id, 14400).catch((error: any) => error?.message || '获取下载地址失败')
      if (typeof data === 'string') {
        result.failed += 1
        result.failures.push({ name: file.name, reason: data })
        continue
      }
      const down = data as IDownloadUrl
      result.links.push({ name: file.name, fileId: file.file_id, driveId: file.drive_id, userId: token.user_id, url: down.url, size: down.size || file.size || 0, headers: down.headers })
      result.success += 1
    }
  }
  result.text = formatDirectLinks(result.links, format)
  return result
}
