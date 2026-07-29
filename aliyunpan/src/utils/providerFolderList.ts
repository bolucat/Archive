import type { IAliGetFileModel } from '../aliapi/alimodels'
import { isBaiduUser, isBoxUser, isCloud139User, isCloud189User, isCloud123User, isDrive115User, isDropboxUser, isGuangyaUser, isOneDriveUser, isPikPakUser, isQuarkUser } from '../aliapi/utils'
import { apiCloud123FileListPage, mapCloud123FileToAliModel } from '../cloud123/dirfilelist'
import { apiDrive115FileList, mapDrive115FileToAliModel } from '../cloud115/dirfilelist'
import { apiBaiduFileList, mapBaiduFileToAliModel } from '../cloudbaidu/dirfilelist'
import { apiPikPakFileList, mapPikPakFileToAliModel } from '../pikpak/dirfilelist'
import { apiDropboxFileList, apiDropboxFileListPage, mapDropboxFileToAliModel } from '../dropbox/dirfilelist'
import { apiOneDriveFileList, apiOneDriveFileListPage, mapOneDriveItemToAliModel } from '../onedrive/dirfilelist'
import { apiBoxFileList, apiBoxFileListPage, mapBoxItemToAliModel } from '../box/dirfilelist'
import { apiQuarkFileList, mapQuarkFileToAliModel } from '../quark/dirfilelist'
import { apiCloud139FileListPage, mapCloud139FileToAliModel } from '../cloud139/dirfilelist'
import { apiCloud189FileList, mapCloud189FileToAliModel } from '../cloud189/dirfilelist'
import { apiGuangyaFileList, apiGuangyaFileListPage, mapGuangyaFileToAliModel } from '../guangya/dirfilelist'

export type ProviderFolderListOptions = {
  folder: IAliGetFileModel
  userId: string
  driveId: string
  silent?: boolean
  shouldStop?: () => boolean
}

export const isThirdPartyProviderFolder = (userId: string, driveId: string): boolean =>
  isCloud123User(userId) || driveId === 'cloud123'
  || isDrive115User(userId) || driveId === 'drive115'
  || isBaiduUser(userId) || driveId === 'baidu'
  || isPikPakUser(userId) || driveId === 'pikpak'
  || isDropboxUser(userId) || driveId === 'dropbox'
  || isOneDriveUser(userId) || driveId === 'onedrive'
  || isBoxUser(userId) || driveId === 'box'
  || isQuarkUser(userId) || driveId === 'quark'
  || isCloud139User(userId) || driveId === 'cloud139'
  || isCloud189User(userId) || driveId === 'cloud189'
  || isGuangyaUser(userId) || driveId === 'guangya'

export async function* iterateProviderFolderPages(options: ProviderFolderListOptions): AsyncGenerator<IAliGetFileModel[]> {
  const { folder, userId, driveId } = options
  const fileId = folder.file_id

  if (isCloud123User(userId) || driveId === 'cloud123') {
    let lastFileId: string | number = ''
    while (!stopped(options)) {
      const page = await apiCloud123FileListPage(userId, fileId || '0', 100, false, '', 0, lastFileId)
      if (page.items.length) yield page.items.map(item => withUser(mapCloud123FileToAliModel(item), userId, driveId))
      if (page.lastFileId < 0 || !page.items.length || String(page.lastFileId) === String(lastFileId)) return
      lastFileId = page.lastFileId
    }
    return
  }
  if (isDrive115User(userId) || driveId === 'drive115') {
    const limit = 200
    for (let offset = 0; !stopped(options); offset += limit) {
      const page = await apiDrive115FileList(userId, fileId || '0', limit, offset, true, { silent: !!options.silent })
      if (page.length) yield page.map(item => withUser(mapDrive115FileToAliModel(item, driveId), userId, driveId))
      if (page.length < limit) return
    }
    return
  }
  if (isBaiduUser(userId) || driveId === 'baidu') {
    const limit = 1000
    const path = (folder as any).path || fileId || '/'
    for (let start = 0; !stopped(options); start += limit) {
      const page = await apiBaiduFileList(userId, path, 'name', start, limit)
      if (page.length) yield page.map(item => withUser(mapBaiduFileToAliModel(item, driveId, fileId || ''), userId, driveId))
      if (page.length < limit) return
    }
    return
  }
  if (isPikPakUser(userId) || driveId === 'pikpak') {
    const parentId = fileId && !fileId.includes('root') ? fileId : 'pikpak_root'
    let pageToken = ''
    const seenTokens = new Set<string>()
    do {
      if (seenTokens.has(pageToken)) return
      seenTokens.add(pageToken)
      const page = await apiPikPakFileList(userId, parentId, 500, pageToken)
      if (page.items.length) yield page.items.map(item => withUser(mapPikPakFileToAliModel(item, driveId, parentId), userId, driveId))
      pageToken = page.nextPageToken
    } while (pageToken && !stopped(options))
    return
  }
  if (isQuarkUser(userId) || driveId === 'quark') {
    const parentId = fileId || '0'
    for (let pageNumber = 1, total = Infinity; pageNumber <= Math.ceil(total / 100) && !stopped(options); pageNumber++) {
      const page = await apiQuarkFileList(userId, parentId, 100, pageNumber)
      total = page.total
      if (page.items.length) yield page.items.map(item => withUser(mapQuarkFileToAliModel(item, driveId, parentId), userId, driveId))
      if (!page.items.length) return
    }
    return
  }
  if (isCloud139User(userId) || driveId === 'cloud139') {
    const parentId = fileId || 'cloud139_root'
    let cursor = ''
    const seenCursors = new Set<string>()
    do {
      if (seenCursors.has(cursor)) return
      seenCursors.add(cursor)
      const page = await apiCloud139FileListPage(userId, parentId, 100, cursor)
      if (page.items.length) yield page.items.map(item => withUser(mapCloud139FileToAliModel(item, driveId, parentId), userId, driveId))
      cursor = page.nextCursor
    } while (cursor && !stopped(options))
    return
  }
  if (isCloud189User(userId) || driveId === 'cloud189') {
    const parentId = fileId || 'cloud189_root'
    const limit = 1000
    for (let pageNum = 1; !stopped(options); pageNum++) {
      const page = await apiCloud189FileList(userId, parentId, limit, pageNum)
      if (page.length) yield page.map(item => withUser(mapCloud189FileToAliModel(item, driveId, parentId), userId, driveId))
      if (page.length < limit) return
    }
    return
  }

  if (isDropboxUser(userId) || driveId === 'dropbox') {
    const parentId = fileId && !fileId.includes('root') ? fileId : 'dropbox_root'
    let cursor = ''
    const seenCursors = new Set<string>()
    do {
      if (seenCursors.has(cursor)) return
      seenCursors.add(cursor)
      const page = await apiDropboxFileListPage(userId, parentId, 500, cursor)
      if (page.items.length) yield page.items.map(item => withUser(mapDropboxFileToAliModel(item, driveId, parentId), userId, driveId))
      cursor = page.hasMore ? page.cursor : ''
    } while (cursor && !stopped(options))
    return
  }
  if (isOneDriveUser(userId) || driveId === 'onedrive') {
    const parentId = fileId && !fileId.includes('root') ? fileId : 'onedrive_root'
    let nextLink = ''
    const seenLinks = new Set<string>()
    do {
      if (seenLinks.has(nextLink)) return
      seenLinks.add(nextLink)
      const page = await apiOneDriveFileListPage(userId, parentId, nextLink)
      if (page.items.length) yield page.items.map(item => withUser(mapOneDriveItemToAliModel(item, driveId, parentId), userId, driveId))
      nextLink = page.nextLink
    } while (nextLink && !stopped(options))
    return
  }
  if (isBoxUser(userId) || driveId === 'box') {
    const parentId = fileId && !fileId.includes('root') ? fileId : 'box_root'
    let offset = 0
    const seenOffsets = new Set<number>()
    while (!stopped(options)) {
      if (seenOffsets.has(offset)) return
      seenOffsets.add(offset)
      const page = await apiBoxFileListPage(userId, parentId, 500, offset)
      if (page.items.length) yield page.items.map(item => withUser(mapBoxItemToAliModel(item, driveId, parentId), userId, driveId))
      if (page.nextOffset === null) return
      offset = page.nextOffset
    }
    return
  }
  if (isGuangyaUser(userId) || driveId === 'guangya') {
    const parentId = fileId || 'guangya_root'
    for (let pageNumber = 0; !stopped(options); pageNumber++) {
      const page = await apiGuangyaFileListPage(userId, parentId, pageNumber)
      if (page.items.length) yield page.items.map(item => withUser(mapGuangyaFileToAliModel(item, driveId, parentId), userId, driveId))
      if (!page.hasMore) return
    }
    return
  }

  const all = await listProviderFolderItems(options)
  if (all?.length) yield all
}

const stopped = (options: ProviderFolderListOptions) => options.shouldStop?.() === true

const withUser = (item: IAliGetFileModel, userId: string, driveId: string): IAliGetFileModel => {
  item.drive_id = item.drive_id || driveId
  ;(item as any).user_id = userId
  return item
}

/**
 * Lists every page of a third-party provider directory and maps it to the
 * common file model. Returns null for Aliyun drives, which keep their own
 * marker-based listing path.
 */
export async function listProviderFolderItems(options: ProviderFolderListOptions): Promise<IAliGetFileModel[] | null> {
  const { folder, userId, driveId } = options
  const fileId = folder.file_id

  if (isCloud123User(userId) || driveId === 'cloud123') {
    const all: any[] = []
    let lastFileId: string | number = ''
    while (!stopped(options)) {
      const page = await apiCloud123FileListPage(userId, fileId || '0', 100, false, '', 0, lastFileId)
      all.push(...page.items)
      if (page.lastFileId < 0 || !page.items.length || String(page.lastFileId) === String(lastFileId)) break
      lastFileId = page.lastFileId
    }
    return all.map(item => withUser(mapCloud123FileToAliModel(item), userId, driveId))
  }

  if (isDrive115User(userId) || driveId === 'drive115') {
    const limit = 200
    const all: any[] = []
    for (let offset = 0; !stopped(options); offset += limit) {
      const page = await apiDrive115FileList(userId, fileId || '0', limit, offset, true, { silent: !!options.silent })
      all.push(...page)
      if (page.length < limit) break
    }
    return all.map(item => withUser(mapDrive115FileToAliModel(item, driveId), userId, driveId))
  }

  if (isBaiduUser(userId) || driveId === 'baidu') {
    const limit = 1000
    const all: any[] = []
    const path = (folder as any).path || fileId || '/'
    for (let start = 0; !stopped(options); start += limit) {
      const page = await apiBaiduFileList(userId, path, 'name', start, limit)
      all.push(...page)
      if (page.length < limit) break
    }
    return all.map(item => withUser(mapBaiduFileToAliModel(item, driveId, fileId || ''), userId, driveId))
  }

  if (isPikPakUser(userId) || driveId === 'pikpak') {
    const parentId = fileId && !fileId.includes('root') ? fileId : 'pikpak_root'
    const all: any[] = []
    let pageToken = ''
    do {
      const page = await apiPikPakFileList(userId, parentId, 500, pageToken)
      all.push(...page.items)
      pageToken = page.nextPageToken
    } while (pageToken && !stopped(options))
    return all.map(item => withUser(mapPikPakFileToAliModel(item, driveId, parentId), userId, driveId))
  }

  if (isDropboxUser(userId) || driveId === 'dropbox') {
    const parentId = fileId && !fileId.includes('root') ? fileId : 'dropbox_root'
    return (await apiDropboxFileList(userId, parentId, 500)).map(item => withUser(mapDropboxFileToAliModel(item, driveId, parentId), userId, driveId))
  }
  if (isOneDriveUser(userId) || driveId === 'onedrive') {
    const parentId = fileId && !fileId.includes('root') ? fileId : 'onedrive_root'
    return (await apiOneDriveFileList(userId, parentId)).map(item => withUser(mapOneDriveItemToAliModel(item, driveId, parentId), userId, driveId))
  }
  if (isBoxUser(userId) || driveId === 'box') {
    const parentId = fileId && !fileId.includes('root') ? fileId : 'box_root'
    return (await apiBoxFileList(userId, parentId, 500)).map(item => withUser(mapBoxItemToAliModel(item, driveId, parentId), userId, driveId))
  }

  if (isQuarkUser(userId) || driveId === 'quark') {
    const parentId = fileId || '0'
    const first = await apiQuarkFileList(userId, parentId, 100, 1)
    const all = [...first.items]
    for (let page = 2; all.length < first.total && !stopped(options); page++) {
      const next = await apiQuarkFileList(userId, parentId, 100, page)
      if (!next.items.length) break
      all.push(...next.items)
    }
    return all.map(item => withUser(mapQuarkFileToAliModel(item, driveId, parentId), userId, driveId))
  }

  if (isCloud139User(userId) || driveId === 'cloud139') {
    const parentId = fileId || 'cloud139_root'
    const all: any[] = []
    let cursor = ''
    do {
      const page = await apiCloud139FileListPage(userId, parentId, 100, cursor)
      all.push(...page.items)
      cursor = page.nextCursor
    } while (cursor && !stopped(options))
    return all.map(item => withUser(mapCloud139FileToAliModel(item, driveId, parentId), userId, driveId))
  }

  if (isCloud189User(userId) || driveId === 'cloud189') {
    const parentId = fileId || 'cloud189_root'
    const limit = 1000
    const all: any[] = []
    for (let pageNum = 1; !stopped(options); pageNum++) {
      const page = await apiCloud189FileList(userId, parentId, limit, pageNum)
      all.push(...page)
      if (page.length < limit) break
    }
    return all.map(item => withUser(mapCloud189FileToAliModel(item, driveId, parentId), userId, driveId))
  }

  if (isGuangyaUser(userId) || driveId === 'guangya') {
    const parentId = fileId || 'guangya_root'
    return (await apiGuangyaFileList(userId, parentId, Number.MAX_SAFE_INTEGER)).map(item => withUser(mapGuangyaFileToAliModel(item, driveId, parentId), userId, driveId))
  }

  return null
}
