import { IAliGetFileModel } from '../aliapi/alimodels'
import AliDirFileList from '../aliapi/dirfilelist'
import AliFileWalk from '../aliapi/filewalk'
import {
  isAliyunUser,
  isBaiduUser,
  isBoxUser,
  isCloud123User,
  isDrive115User,
  isDropboxUser,
  isOneDriveUser,
  isPikPakUser
} from '../aliapi/utils'
import UserDAL from '../user/userdal'
import { ITokenInfo } from '../user/userstore'
import useMusicLibraryStore from '../store/musiclibrary'
import useSettingStore from '../setting/settingstore'
import { IMusicTrack } from '../types/music'
import DebugLog from './debuglog'

import { apiCloud123FileList, mapCloud123FileToAliModel } from '../cloud123/dirfilelist'
import { apiDrive115FileList, mapDrive115FileToAliModel } from '../cloud115/dirfilelist'
import { apiBaiduFileList, mapBaiduFileToAliModel } from '../cloudbaidu/dirfilelist'
import { apiPikPakFileList, mapPikPakFileToAliModel } from '../pikpak/dirfilelist'
import { apiDropboxFileList, mapDropboxFileToAliModel } from '../dropbox/dirfilelist'
import { apiOneDriveFileList, mapOneDriveItemToAliModel } from '../onedrive/dirfilelist'
import { apiBoxFileList, mapBoxItemToAliModel } from '../box/dirfilelist'

const AUDIO_EXTS = new Set([
  '.mp3', '.flac', '.wav', '.ape', '.ogg', '.aac',
  '.aif', '.aiff', '.cda', '.dsf', '.dts', '.dtshd',
  '.eac3', '.m1a', '.m2a', '.m4a', '.m4b', '.m4r',
  '.mka', '.mpa', '.mpc', '.opus', '.ra', '.tak',
  '.tta', '.wma', '.wv', '.amr', '.ac3', '.au'
])

const FOLDER_THROTTLE_MS = 60
const BFS_MAX_DEPTH = 8

function nowMs() { return Date.now() }

function isAudioFile(item: IAliGetFileModel): boolean {
  if (!item || item.isDir) return false
  if (item.category && (item.category === 'audio' || item.category === 'audio2')) return true
  const name = item.name || ''
  const dot = name.lastIndexOf('.')
  if (dot < 0) return false
  return AUDIO_EXTS.has(name.slice(dot).toLowerCase())
}

function trackFromAliModel(item: IAliGetFileModel, user_id: string, drive_id: string, parent_path: string): IMusicTrack {
  const id = `${user_id}|${drive_id}|${item.file_id}`
  const ext = item.ext || (() => {
    const i = (item.name || '').lastIndexOf('.')
    return i >= 0 ? (item.name || '').slice(i + 1) : ''
  })()
  return {
    id,
    user_id,
    drive_id,
    file_id: item.file_id,
    parent_file_id: item.parent_file_id || '',
    parent_path: parent_path || '',
    file_name: item.name || '',
    ext: ext || '',
    size: item.size || 0,
    category: (item.category as any) || 'audio',
    thumbnail: item.thumbnail || '',
    description: item.description || '',
    encType: item.description && item.description.includes('xbyEncrypt') ? 'xbyEncrypt' : '',
    scanned_at: nowMs(),
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
    default: return token.nick_name || token.user_name || token.user_id
  }
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

class MusicScanner {
  private static instance: MusicScanner | null = null
  private isRunning = false
  private shouldStop = false

  static getInstance(): MusicScanner {
    if (!MusicScanner.instance) MusicScanner.instance = new MusicScanner()
    return MusicScanner.instance
  }

  get isScanning(): boolean { return this.isRunning }

  stopScan(): void {
    this.shouldStop = true
  }

  /**
   * 手动扫描指定文件夹下所有音频文件（递归 BFS）。
   * 与 scanAllUsers 互斥（共用 isRunning 标志）。
   */
  async scanFolder(folder: IAliGetFileModel, user_id: string): Promise<{ found: number; scanned: number }> {
    if (this.isRunning) {
      return { found: 0, scanned: 0 }
    }
    this.isRunning = true
    this.shouldStop = false
    const store = useMusicLibraryStore()
    store.setIsScanning(true)
    const drive_id = folder.drive_id || ''
    const label = folder.name || '指定文件夹'
    store.setScanProgress(`正在扫描 ${label}`, 0, 0)
    const counters = { scanned: 0, found: 0 }
    try {
      await this.bfsCollect(folder, user_id, drive_id, folder.name || '', label, counters, 0)
      store.markScanFinished()
      // 注册到自动扫描列表，下次启动 app 时也做增量
      this.registerAutoScanFolder({
        user_id,
        drive_id,
        file_id: folder.file_id,
        name: folder.name || '',
        path: (folder as any).path || ''
      }).catch(() => { /* ignore */ })
    } catch (e) {
      DebugLog.mSaveWarning('musicScanner.scanFolder failed: ' + (e as Error).message)
    } finally {
      store.setIsScanning(false)
      this.isRunning = false
      this.shouldStop = false
    }
    return counters
  }

  /**
   * 扫描设置中已注册的所有手动扫描文件夹（来自 setting.uiMusicAutoScanFolders）。
   * 与 scanAllUsers 互斥。每个文件夹独立 BFS，并按 sinceMs 增量过滤（在 bfsCollect 内未过滤，
   * 因为去重靠 musicStore.appendTracks 的 id 合并）。
   */
  async scanRegisteredFolders(opts: {
    folders: { user_id: string; drive_id: string; file_id: string; name?: string; path?: string }[]
    userIdAllowList?: Set<string>
  }): Promise<{ scanned: number; found: number; folderCount: number }> {
    const summary = { scanned: 0, found: 0, folderCount: 0 }
    if (!opts.folders.length) return summary
    if (this.isRunning) return summary
    this.isRunning = true
    this.shouldStop = false
    const store = useMusicLibraryStore()
    store.setIsScanning(true)
    try {
      for (const f of opts.folders) {
        if (this.shouldStop) break
        if (opts.userIdAllowList && !opts.userIdAllowList.has(f.user_id)) continue
        const folderModel: IAliGetFileModel = {
          ...({} as any),
          file_id: f.file_id,
          parent_file_id: '',
          drive_id: f.drive_id,
          name: f.name || '',
          isDir: true
        } as IAliGetFileModel
        ;(folderModel as any).path = f.path || ''
        store.setScanProgress(`正在扫描 ${f.name || '指定文件夹'}`, summary.scanned, summary.found)
        const counters = { scanned: 0, found: 0 }
        try {
          await this.bfsCollect(folderModel, f.user_id, f.drive_id, f.name || '', f.name || '指定文件夹', counters, 0)
          summary.scanned += counters.scanned
          summary.found += counters.found
          summary.folderCount += 1
        } catch (e) {
          DebugLog.mSaveWarning('musicScanner.scanRegisteredFolders item failed: ' + (e as Error).message)
        }
      }
      store.markScanFinished()
    } finally {
      store.setIsScanning(false)
      this.isRunning = false
      this.shouldStop = false
    }
    return summary
  }

  private async registerAutoScanFolder(entry: { user_id: string; drive_id: string; file_id: string; name: string; path?: string }) {
    if (!entry.user_id || !entry.drive_id || !entry.file_id) return
    const setting = useSettingStore()
    const list = (setting.uiMusicAutoScanFolders || []).slice()
    // 用 user_id|drive_id|file_id 作为唯一键
    const key = (e: typeof entry) => `${e.user_id}|${e.drive_id}|${e.file_id}`
    const existing = new Set(list.map(key))
    if (existing.has(key(entry))) return
    list.push({
      user_id: entry.user_id,
      drive_id: entry.drive_id,
      file_id: entry.file_id,
      name: entry.name,
      path: entry.path || ''
    })
    await setting.updateStore({ uiMusicAutoScanFolders: list })
  }

  async scanAllUsers(forceOrOpts: boolean | { force?: boolean; sinceMs?: number; userIdAllowList?: Set<string> } = false): Promise<void> {
    const opts = typeof forceOrOpts === 'boolean' ? { force: forceOrOpts } : forceOrOpts
    const force = !!opts.force
    const sinceMs = opts.sinceMs || 0
    const allowList = opts.userIdAllowList
    if (this.isRunning) return
    this.isRunning = true
    this.shouldStop = false
    const store = useMusicLibraryStore()
    store.setIsScanning(true)
    try {
      const users = await UserDAL.GetUserListFromDB()
      if (!users.length) return
      for (const u of users) {
        if (this.shouldStop) break
        if (!u || !u.access_token) continue
        if (allowList && !allowList.has(u.user_id)) continue
        try {
          await this.scanUser(u, force, sinceMs)
        } catch (e) {
          DebugLog.mSaveWarning('musicScanner.scanUser failed: ' + (e as Error).message)
        }
      }
      store.markScanFinished()
    } finally {
      store.setIsScanning(false)
      this.isRunning = false
      this.shouldStop = false
    }
  }

  async scanUser(token: ITokenInfo, _force: boolean = false, sinceMs: number = 0): Promise<void> {
    if (!token || !token.access_token) return
    const label = readableTokenLabel(token)
    const store = useMusicLibraryStore()
    store.setScanProgress(`正在扫描 ${label}`, 0, 0)

    if (token.tokenfrom === 'aliyun' || isAliyunUser(token.user_id)) {
      await this.scanAliyun(token, label, sinceMs)
      return
    }
    // 非阿里云盘：BFS
    const sentinelMap: Record<string, string> = {
      'cloud123': '0',
      '115': '0',
      'baidu': '/',
      'pikpak': 'pikpak_root',
      'dropbox': 'dropbox_root',
      'onedrive': 'onedrive_root',
      'box': '0'
    }
    const driveId =
      token.tokenfrom === 'cloud123' ? 'cloud123' :
      token.tokenfrom === '115' ? 'drive115' :
      token.tokenfrom === 'baidu' ? 'baidu' :
      token.tokenfrom === 'pikpak' ? 'pikpak' :
      token.tokenfrom === 'dropbox' ? 'dropbox' :
      token.tokenfrom === 'onedrive' ? 'onedrive' :
      token.tokenfrom === 'box' ? 'box' :
      ''
    if (!driveId) return
    const rootId = sentinelMap[token.tokenfrom] || ''
    const rootFolder: IAliGetFileModel = {
      ...({} as any),
      file_id: rootId,
      parent_file_id: '',
      drive_id: driveId,
      name: '/',
      isDir: true
    } as IAliGetFileModel
    ;(rootFolder as any).path = token.tokenfrom === 'baidu' ? '/' : ''
    const counters = { scanned: 0, found: 0 }
    await this.bfsCollect(rootFolder, token.user_id, driveId, '', label, counters, 0)
  }

  // ============== Aliyun Server-side Walk ==============

  private async scanAliyun(token: ITokenInfo, label: string, sinceMs: number = 0): Promise<void> {
    const drives = new Set<string>()
    if (token.default_drive_id) drives.add(token.default_drive_id)
    if (token.resource_drive_id) drives.add(token.resource_drive_id)
    if (token.backup_drive_id) drives.add(token.backup_drive_id)
    const store = useMusicLibraryStore()
    let totalFound = 0
    let scanned = 0
    for (const drive_id of drives) {
      if (this.shouldStop) break
      try {
        const driveLabel = drive_id.slice(-6)
        store.setScanProgress(`正在扫描 ${label} · drive ${driveLabel}`, scanned, totalFound)
        const resp = await AliFileWalk.ApiWalkFileList(
          token.user_id,
          drive_id,
          'root',
          '',
          'updated_at desc',
          'audio',
          0
        )
        const items = resp?.items || []
        scanned += items.length
        // 增量：updated_at 已是 desc，遇到第一个旧于 sinceMs 的就停止整页处理
        let filtered = items.filter(isAudioFile)
        if (sinceMs > 0) {
          const cut: typeof filtered = []
          for (const it of filtered) {
            const t = typeof it.time === 'number' ? it.time : 0
            if (t && t < sinceMs) break
            cut.push(it)
          }
          filtered = cut
        }
        if (filtered.length) {
          const tracks = filtered.map((it) => trackFromAliModel(it, token.user_id, drive_id, ''))
          await store.appendTracks(tracks)
          totalFound += tracks.length
        }
        store.setScanProgress(`正在扫描 ${label} · drive ${driveLabel}`, scanned, totalFound)
      } catch (e) {
        DebugLog.mSaveWarning('aliyun walk failed: ' + (e as Error).message)
      }
    }
  }

  // ============== Cross-cloud BFS ==============

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
    const store = useMusicLibraryStore()
    let items: IAliGetFileModel[] = []
    try {
      items = await this.listFolder(folder, user_id, drive_id)
    } catch (e) {
      DebugLog.mSaveWarning('listFolder failed: ' + (e as Error).message)
      return
    }
    counters.scanned += items.length
    const audios = items.filter(isAudioFile)
    if (audios.length) {
      const tracks = audios.map((it) => trackFromAliModel(it, user_id, drive_id, parent_path))
      await store.appendTracks(tracks)
      counters.found += tracks.length
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

  private async listFolder(
    folder: IAliGetFileModel,
    user_id: string,
    drive_id: string
  ): Promise<IAliGetFileModel[]> {
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
      const list = await apiDrive115FileList(user_id, fileId || '0', 500, 0, true)
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

export default MusicScanner
