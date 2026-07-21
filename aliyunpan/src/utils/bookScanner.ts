import { IAliGetFileModel } from '../aliapi/alimodels'
import AliDirFileList from '../aliapi/dirfilelist'
import {
  isAliyunUser,
  isBaiduUser,
  isBoxUser,
  isCloud123User,
  isCloud139User,
  isCloud189User,
  isDrive115User,
  isDropboxUser,
  isGuangyaUser,
  isOneDriveUser,
  isPikPakUser,
  isQuarkUser
} from '../aliapi/utils'
import UserDAL from '../user/userdal'
import { ITokenInfo } from '../user/userstore'
import useBookLibraryStore, { parseBookMeta } from '../store/booklibrary'
import { IBookItem } from '../types/book'
import { isScannableBookFormat } from './bookReaderCapabilities'
import DebugLog from './debuglog'

const ISBN_RE = /(?:ISBN(?:-1[03])?:?\s*)?((?:97[89][-\s]?)?\d[-\s]?\d{2,5}[-\s]?\d{2,7}[-\s]?\d{1,7}[-\s]?[\dXx])/g

function extractISBN(...values: Array<string | undefined>): string {
  for (const value of values) {
    ISBN_RE.lastIndex = 0
    let match: RegExpExecArray | null
    while ((match = ISBN_RE.exec(String(value || '')))) {
      const isbn = String(match[1] || '').replace(/[^0-9Xx]/g, '').toUpperCase()
      if (isbn.length === 10 || isbn.length === 13) return isbn
    }
  }
  return ''
}

import { apiCloud123FileList, mapCloud123FileToAliModel } from '../cloud123/dirfilelist'
import { apiDrive115FileList, mapDrive115FileToAliModel } from '../cloud115/dirfilelist'
import { apiBaiduFileList, mapBaiduFileToAliModel } from '../cloudbaidu/dirfilelist'
import { apiPikPakFileList, mapPikPakFileToAliModel } from '../pikpak/dirfilelist'
import { apiDropboxFileList, mapDropboxFileToAliModel } from '../dropbox/dirfilelist'
import { apiOneDriveFileList, mapOneDriveItemToAliModel } from '../onedrive/dirfilelist'
import { apiBoxFileList, mapBoxItemToAliModel } from '../box/dirfilelist'
import { apiQuarkFileList, mapQuarkFileToAliModel } from '../quark/dirfilelist'
import { apiCloud139FileList, mapCloud139FileToAliModel } from '../cloud139/dirfilelist'
import { apiCloud189FileList, mapCloud189FileToAliModel } from '../cloud189/dirfilelist'
import { apiGuangyaFileList, mapGuangyaFileToAliModel } from '../guangya/dirfilelist'

const FOLDER_THROTTLE_MS = 50
const BFS_MAX_DEPTH = 8

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

function isBookFile(item: IAliGetFileModel): boolean {
  if (!item || item.isDir) return false
  const name = item.name || ''
  const dot = name.lastIndexOf('.')
  if (dot < 0) return false
  return isScannableBookFormat(name.slice(dot + 1))
}

function bookFromAliModel(item: IAliGetFileModel, user_id: string, drive_id: string, parent_path: string): IBookItem {
  const ext = item.ext || (() => {
    const i = (item.name || '').lastIndexOf('.')
    return i >= 0 ? (item.name || '').slice(i + 1) : ''
  })()
  const meta = parseBookMeta(item.name || '')
  const upperExt = (ext || '').toUpperCase()
  const isbn = extractISBN(item.name, item.description)
  return {
    id: `${user_id}|${drive_id}|${item.file_id}`,
    user_id,
    drive_id,
    file_id: item.file_id,
    parent_file_id: item.parent_file_id || '',
    parent_path: parent_path || '',
    file_name: item.name || '',
    ext: ext || '',
    size: item.size || 0,
    category: (item.category as any) || 'book',
    thumbnail: item.thumbnail || '',
    description: item.description || '',
    encType: item.description && item.description.includes('xbyEncrypt') ? 'xbyEncrypt' : '',
    title: meta.title,
    author: meta.author,
    summary: meta.summary || `${upperExt || 'BOOK'} · ${parent_path || '根目录'}`,
    cover_url: item.thumbnail || '',
    isbn,
    metadata_source: item.thumbnail ? 'thumbnail' : meta.metadata_source,
    scanned_at: Date.now(),
    updated_at: typeof item.time === 'number' && item.time > 0 ? item.time : undefined
  }
}

function readableTokenLabel(token: ITokenInfo): string {
  switch (token.tokenfrom) {
    case 'aliyun': return `阿里云盘 · ${token.nick_name || token.user_name || ''}`.trim()
    case 'cloud123': return `123 网盘 · ${token.nick_name || token.user_name || ''}`.trim()
    case '115': return `115 网盘 · ${token.nick_name || token.user_name || ''}`.trim()
    case 'baidu': return `百度网盘 · ${token.nick_name || token.user_name || ''}`.trim()
    case 'pikpak': return `PikPak · ${token.nick_name || token.user_name || ''}`.trim()
    case 'dropbox': return `Dropbox · ${token.nick_name || token.user_name || ''}`.trim()
    case 'onedrive': return `OneDrive · ${token.nick_name || token.user_name || ''}`.trim()
    case 'box': return `Box · ${token.nick_name || token.user_name || ''}`.trim()
    case 'quark': return `夸克网盘 · ${token.nick_name || token.user_name || ''}`.trim()
    case '139': return `139 云盘 · ${token.nick_name || token.user_name || ''}`.trim()
    case '189': return `天翼云盘 · ${token.nick_name || token.user_name || ''}`.trim()
    default: return token.nick_name || token.user_name || token.user_id
  }
}

class BookScanner {
  private static instance: BookScanner | null = null
  private isRunning = false
  private shouldStop = false
  private silent = false

  static getInstance(): BookScanner {
    if (!BookScanner.instance) BookScanner.instance = new BookScanner()
    return BookScanner.instance
  }

  get isScanning(): boolean { return this.isRunning }

  stopScan(): void {
    this.shouldStop = true
  }

  async scanFolder(folder: IAliGetFileModel, user_id: string): Promise<{ found: number; scanned: number }> {
    if (this.isRunning) return { found: 0, scanned: 0 }
    this.isRunning = true
    this.shouldStop = false
    this.silent = false
    const store = useBookLibraryStore()
    const drive_id = folder.drive_id || ''
    const label = folder.name || '指定文件夹'
    const counters = { scanned: 0, found: 0 }
    store.setIsScanning(true)
    store.setScanProgress(`正在扫描 ${label}`, 0, 0)
    try {
      await this.bfsCollect(folder, user_id, drive_id, folder.name || '', label, counters, 0)
      await store.markScanFinished()
    } catch (e) {
      DebugLog.mSaveWarning('bookScanner.scanFolder failed: ' + (e as Error).message)
    } finally {
      store.setIsScanning(false)
      this.isRunning = false
      this.shouldStop = false
      this.silent = false
    }
    return counters
  }

  async scanAllUsers(opts: { userIdAllowList?: Set<string>; silent?: boolean } = {}): Promise<void> {
    if (this.isRunning) return
    this.isRunning = true
    this.shouldStop = false
    this.silent = !!opts.silent
    const store = useBookLibraryStore()
    store.setIsScanning(true)
    try {
      const users = await UserDAL.GetUserListFromDB()
      for (const u of users) {
        if (this.shouldStop) break
        if (!u || !u.access_token) continue
        if (opts.userIdAllowList && !opts.userIdAllowList.has(u.user_id)) continue
        try {
          await this.scanUser(u)
        } catch (e) {
          DebugLog.mSaveWarning('bookScanner.scanUser failed: ' + (e as Error).message)
        }
      }
      await store.markScanFinished()
    } finally {
      store.setIsScanning(false)
      this.isRunning = false
      this.shouldStop = false
      this.silent = false
    }
  }

  async scanUser(token: ITokenInfo): Promise<void> {
    if (!token || !token.access_token) return
    const label = readableTokenLabel(token)
    const store = useBookLibraryStore()
    store.setScanProgress(`正在扫描 ${label}`, 0, 0)

    if (token.tokenfrom === 'aliyun' || isAliyunUser(token.user_id)) {
      await this.scanAliyun(token, label)
      return
    }

    const sentinelMap: Record<string, string> = {
      cloud123: '0',
      '115': '0',
      baidu: '/',
      pikpak: 'pikpak_root',
      dropbox: 'dropbox_root',
      onedrive: 'onedrive_root',
      box: '0',
      quark: '0',
      '139': 'cloud139_root',
      '189': 'cloud189_root',
      guangya: 'guangya_root'
    }
    const driveId =
      token.tokenfrom === 'cloud123' ? 'cloud123' :
      token.tokenfrom === '115' ? 'drive115' :
      token.tokenfrom === 'baidu' ? 'baidu' :
      token.tokenfrom === 'pikpak' ? 'pikpak' :
      token.tokenfrom === 'dropbox' ? 'dropbox' :
      token.tokenfrom === 'onedrive' ? 'onedrive' :
      token.tokenfrom === 'box' ? 'box' :
      token.tokenfrom === 'quark' ? 'quark' :
      token.tokenfrom === '139' ? 'cloud139' :
      token.tokenfrom === '189' ? 'cloud189' :
      token.tokenfrom === 'guangya' ? 'guangya' :
      ''
    if (!driveId) return

    const rootFolder: IAliGetFileModel = {
      ...({} as any),
      file_id: sentinelMap[token.tokenfrom] || '',
      parent_file_id: '',
      drive_id: driveId,
      name: '/',
      isDir: true
    } as IAliGetFileModel
    ;(rootFolder as any).path = token.tokenfrom === 'baidu' ? '/' : ''
    const counters = { scanned: 0, found: 0 }
    await this.bfsCollect(rootFolder, token.user_id, driveId, '', label, counters, 0)
  }

  private async scanAliyun(token: ITokenInfo, label: string): Promise<void> {
    const drives = new Set<string>()
    if (token.default_drive_id) drives.add(token.default_drive_id)
    if (token.resource_drive_id) drives.add(token.resource_drive_id)
    if (token.backup_drive_id) drives.add(token.backup_drive_id)
    const store = useBookLibraryStore()
    let scanned = 0
    let found = 0
    for (const drive_id of drives) {
      if (this.shouldStop) break
      const rootFolder: IAliGetFileModel = {
        ...({} as any),
        file_id: 'root',
        parent_file_id: '',
        drive_id,
        name: '/',
        isDir: true
      } as IAliGetFileModel
      const counters = { scanned: 0, found: 0 }
      await this.bfsCollect(rootFolder, token.user_id, drive_id, '', `${label} · ${drive_id.slice(-6)}`, counters, 0)
      scanned += counters.scanned
      found += counters.found
      store.setScanProgress(`正在扫描 ${label}`, scanned, found)
    }
  }

  private async bfsCollect(
    folder: IAliGetFileModel,
    user_id: string,
    drive_id: string,
    parent_path: string,
    label: string,
    counters: { scanned: number; found: number },
    depth: number
  ): Promise<void> {
    if (this.shouldStop || depth > BFS_MAX_DEPTH) return
    const store = useBookLibraryStore()
    let items: IAliGetFileModel[] = []
    try {
      items = await this.listFolder(folder, user_id, drive_id)
    } catch (e) {
      DebugLog.mSaveWarning('bookScanner.listFolder failed: ' + (e as Error).message)
      return
    }
    counters.scanned += items.length
    const books = items.filter(isBookFile)
    if (books.length) {
      await store.appendBooks(books.map((it) => bookFromAliModel(it, user_id, drive_id, parent_path)), { addToLoaded: false })
      counters.found += books.length
    }
    store.setScanProgress(`正在扫描 ${label}`, counters.scanned, counters.found)

    for (const child of items) {
      if (this.shouldStop) break
      if (!child.isDir) continue
      const childPath = parent_path ? `${parent_path}/${child.name}` : child.name
      await delay(FOLDER_THROTTLE_MS)
      await this.bfsCollect(child, user_id, drive_id, childPath, label, counters, depth + 1)
    }
  }

  private async listFolder(folder: IAliGetFileModel, user_id: string, drive_id: string): Promise<IAliGetFileModel[]> {
    const fileId = folder.file_id

    if (isCloud123User(user_id) || drive_id === 'cloud123') {
      const list = await apiCloud123FileList(user_id, fileId || '0', 100)
      return list.map((item: any) => {
        const mapped = mapCloud123FileToAliModel(item)
        mapped.drive_id = drive_id
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isDrive115User(user_id) || drive_id === 'drive115') {
      const list = await apiDrive115FileList(user_id, fileId || '0', 500, 0, true, { silent: this.silent })
      return list.map((item: any) => {
        const mapped = mapDrive115FileToAliModel(item, drive_id)
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isBaiduUser(user_id) || drive_id === 'baidu') {
      const parentPath = (folder as any).path || folder.file_id || '/'
      const list = await apiBaiduFileList(user_id, parentPath, 'name', 0, 1000)
      return list.map((item: any) => {
        const mapped = mapBaiduFileToAliModel(item, drive_id, folder.file_id || '')
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isPikPakUser(user_id) || drive_id === 'pikpak') {
      const parentId = fileId && !fileId.includes('root') ? fileId : 'pikpak_root'
      const list = await apiPikPakFileList(user_id, parentId, 500)
      return (list?.items || []).map((item: any) => {
        const mapped = mapPikPakFileToAliModel(item, drive_id, parentId)
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isDropboxUser(user_id) || drive_id === 'dropbox') {
      const parentId = fileId && !fileId.includes('root') ? fileId : 'dropbox_root'
      const list = await apiDropboxFileList(user_id, parentId, 500)
      return list.map((item: any) => {
        const mapped = mapDropboxFileToAliModel(item, drive_id, parentId)
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isOneDriveUser(user_id) || drive_id === 'onedrive') {
      const parentId = fileId && !fileId.includes('root') ? fileId : 'onedrive_root'
      const list = await apiOneDriveFileList(user_id, parentId)
      return list.map((item: any) => {
        const mapped = mapOneDriveItemToAliModel(item, drive_id, parentId)
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isBoxUser(user_id) || drive_id === 'box') {
      const parentId = fileId && !fileId.includes('root') ? fileId : 'box_root'
      const list = await apiBoxFileList(user_id, parentId, 500)
      return list.map((item: any) => {
        const mapped = mapBoxItemToAliModel(item, drive_id, parentId)
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isQuarkUser(user_id) || drive_id === 'quark') {
      const resp = await apiQuarkFileList(user_id, fileId || '0', 100, 1)
      return resp.items.map((item: any) => {
        const mapped = mapQuarkFileToAliModel(item, drive_id, fileId || '0')
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isCloud139User(user_id) || drive_id === 'cloud139') {
      const list = await apiCloud139FileList(user_id, fileId || 'cloud139_root', 100)
      return list.map((item: any) => {
        const mapped = mapCloud139FileToAliModel(item, drive_id, fileId || 'cloud139_root')
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isCloud189User(user_id) || drive_id === 'cloud189') {
      const list = await apiCloud189FileList(user_id, fileId || 'cloud189_root', 1000)
      return list.map((item: any) => {
        const mapped = mapCloud189FileToAliModel(item, drive_id, fileId || 'cloud189_root')
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isGuangyaUser(user_id) || drive_id === 'guangya') {
      const list = await apiGuangyaFileList(user_id, fileId || 'guangya_root', 200)
      return list.map((item: any) => {
        const mapped = mapGuangyaFileToAliModel(item, drive_id, fileId || 'guangya_root')
        ;(mapped as any).user_id = user_id
        return mapped
      })
    }
    if (isAliyunUser(user_id)) {
      const result = await AliDirFileList.ApiDirFileList(
        user_id,
        drive_id,
        fileId,
        folder.name || '',
        'name asc',
        '',
        undefined,
        false
      )
      return result?.items || []
    }
    return []
  }
}

export default BookScanner
