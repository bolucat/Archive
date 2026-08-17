import { IAliGetDirModel } from '../aliapi/alimodels'
import AliFile from '../aliapi/file'
import AliDirFileList, { NewIAliFileResp } from '../aliapi/dirfilelist'
import { ITokenInfo, useFootStore, usePanFileStore } from '../store'
import TreeStore, { IDriverModel, TreeNodeData } from '../store/treestore'
import DB from '../utils/db'
import DebugLog from '../utils/debuglog'
import message from '../utils/message'
import usePanTreeStore from './pantreestore'
import { getDriveId as GetDriveID, getDriveType as GetDriveType } from '../drive/context'
import { isBaiduUser, isCloud123User } from '../utils/driveIdentity'
import AliAlbum from '../aliapi/album'
import { getWebDavConnection, getWebDavConnectionId, isWebDavDrive, listWebDavDirectory } from '../utils/webdavClient'
import { resolveDriveProvider, type DriveProvider } from '../utils/driveProvider'
import { listProviderItems } from '../drive/providerList'
import { getProviderFileInfo } from '../drive/providerFile'
import { OrderDir } from '../utils/filenameorder'
import UserDAL from '../user/userdal'
import { buildQuickFilePath, mergeQuickFiles, migrateLegacyQuickFiles, QUICK_FILE_STORAGE_KEY, type LegacyQuickFileEntry, type QuickFileEntry } from './quickFiles'

export interface PanSelectedData {
  isError: boolean
  isErrorSelected: boolean
  user_id: string
  drive_id: string
  dirID: string
  albumId: string
  parentDirID: string
  fileDescription: string
  parentDirDescription: string
  selectedKeys: string[]
  selectedParentKeys: string[],
}

const RefreshLock = new Set<string>()
const AllDirLoadingDrives = new Set<string>()
const AllDirLoadingTimers = new Map<string, number>()
const ALL_DIR_LOADING_TIMEOUT = 5 * 60 * 1000

const beginAllDirLoading = (drive_id: string) => {
  AllDirLoadingDrives.add(drive_id)
  useFootStore().mSaveLoading('加载全部文件夹...')
  if (AllDirLoadingTimers.has(drive_id)) return
  const timer = window.setTimeout(() => {
    AllDirLoadingTimers.delete(drive_id)
    AllDirLoadingDrives.delete(drive_id)
    if (AllDirLoadingDrives.size === 0) useFootStore().mSaveLoading('')
  }, ALL_DIR_LOADING_TIMEOUT)
  AllDirLoadingTimers.set(drive_id, timer)
}

const finishAllDirLoading = (drive_id: string) => {
  const timer = AllDirLoadingTimers.get(drive_id)
  if (timer !== undefined) window.clearTimeout(timer)
  AllDirLoadingTimers.delete(drive_id)
  AllDirLoadingDrives.delete(drive_id)
  useFootStore().mSaveLoading(AllDirLoadingDrives.size > 0 ? '加载全部文件夹...' : '')
}

export default class PanDAL {
  private static providerNextCursor = new Map<string, string>()
  private static providerLoadingMore = new Set<string>()

  private static saveProviderCursor(userId: string, driveId: string, dirId: string, hasFiles: boolean, cursor?: string) {
    const key = `${userId}:${driveId}:${dirId}`
    if (hasFiles && cursor) PanDAL.providerNextCursor.set(key, cursor)
    else PanDAL.providerNextCursor.delete(key)
  }

  static async LoadMoreCurrentProviderItems(): Promise<void> {
    const store = usePanFileStore()
    const tree = usePanTreeStore()
    const key = `${tree.user_id}:${store.DriveID}:${store.DirID}`
    const cursor = PanDAL.providerNextCursor.get(key)
    if (!cursor || PanDAL.providerLoadingMore.has(key)) return
    const route = resolveDriveProvider(tree.user_id, store.DriveID, UserDAL.GetUserToken(tree.user_id)?.tokenfrom)
    if (route.provider !== 'cloud123' && route.provider !== '115' && route.provider !== 'baidu' && route.provider !== 'pikpak' && route.provider !== 'quark' && route.provider !== '139' && route.provider !== '189' && route.provider !== 'guangya' && route.provider !== 'box' && route.provider !== 'dropbox' && route.provider !== 'onedrive' && route.provider !== 'google') return
    PanDAL.providerLoadingMore.add(key)
    try {
      const result = await listProviderItems(route.provider, tree.user_id, store.DriveID, store.DirID, true, cursor)
      if (!result || store.DriveID !== tree.drive_id || store.DirID !== tree.selectDir.file_id) return
      const existing = new Set(store.ListDataRaw.map((item: any) => item.file_id))
      const items = result.items.filter(item => !existing.has(item.file_id))
      if (items.length) {
        store.ListDataRaw = store.ListDataRaw.concat(items)
        const order = TreeStore.GetDirOrder(store.DriveID, store.DirID).replace('ext ', 'updated_at ').split(' ')
        OrderDir(order[0], order[1], store.ListDataRaw)
        store.mRefreshListDataShow(true)
      }
      if (result.nextCursor) PanDAL.providerNextCursor.set(key, result.nextCursor)
      else PanDAL.providerNextCursor.delete(key)
    } finally {
      PanDAL.providerLoadingMore.delete(key)
    }
  }

  private static async SaveProviderDirFileList(userId: string, driveId: string, dirId: string, dirName: string, items: any[], total: number, hasFiles: boolean): Promise<void> {
    const dir = NewIAliFileResp(userId, driveId, dirId, dirName)
    dir.items = items
    dir.itemsKey = new Set(items.map(item => item.file_id))
    dir.next_marker = ''
    dir.itemsTotal = total
    const panfileStore = usePanFileStore()
    panfileStore.mSaveDirFileLoadingPart(0, dir, dir.itemsTotal || 0)
    await TreeStore.SaveOneDirFileList(dir, hasFiles)
    if (hasFiles) panfileStore.mSaveDirFileLoadingFinish(driveId, dirId, dir.items, dir.itemsTotal || 0)
    const panTreeStore = usePanTreeStore()
    if (panTreeStore.user_id === userId && panTreeStore.drive_id === driveId && !panTreeStore.selectDir.file_id) {
      const currentDir: IAliGetDirModel = {
        __v_skip: true,
        drive_id: driveId,
        file_id: dirId,
        parent_file_id: '',
        name: dirName,
        namesearch: '',
        size: 0,
        time: 0,
        description: ''
      }
      panTreeStore.mShowDir(currentDir, [currentDir], [dirId], [dirId])
    }
    PanDAL.RefreshPanTreeAllNode(driveId)
  }

  private static async aReLoadProviderDrive(token: ITokenInfo, provider: DriveProvider, driveId: string, loadingText: string, rootId: string): Promise<void> {
    const { user_id } = token
    const pantreeStore = usePanTreeStore()
    pantreeStore.mSaveUser(user_id, driveId, '', '', '')
    pantreeStore.drive_id = driveId
    if (!user_id) return
    useFootStore().mSaveLoading(loadingText)
    const driveType = GetDriveType(user_id, driveId)
    const result = await listProviderItems(provider, user_id, driveId, rootId, false)
    const dirs = (result?.items || []).map(item => ({ file_id: item.file_id, drive_id: driveId, parent_file_id: driveType.key, name: item.name, description: item.description || '', time: item.time, size: 0 })) as IAliGetDirModel[]
    await TreeStore.ConvertToOneDriver(user_id, driveId, dirs, false, true)
    PanDAL.RefreshPanTreeAllNode(driveId)
    useFootStore().mSaveLoading('')
  }

  static async aReLoadWebDavDrive(token: ITokenInfo): Promise<void> {
    const drive_id = token.default_drive_id
    const pantreeStore = usePanTreeStore()
    pantreeStore.mSaveUser(token.user_id, drive_id, '', '', '')
    pantreeStore.drive_id = drive_id
    await TreeStore.ConvertToOneDriver(token.user_id, drive_id, [], false, true)
    PanDAL.RefreshPanTreeAllNode(drive_id)
  }

  static async aReLoadCloudDrive(token: ITokenInfo): Promise<void> {
    await PanDAL.aReLoadProviderDrive(token, 'cloud123', token.default_drive_id || token.resource_drive_id || 'cloud123', '加载 123 网盘文件夹...', 'cloud_root')
  }

  static async aReLoadDrive115(token: ITokenInfo): Promise<void> {
    await PanDAL.aReLoadProviderDrive(token, '115', token.default_drive_id || 'drive115', '加载 115 网盘文件夹...', 'drive115_root')
  }

  static async aReLoadBaiduDrive(token: ITokenInfo): Promise<void> {
    await PanDAL.aReLoadProviderDrive(token, 'baidu', token.default_drive_id || 'baidu', '加载 百度网盘文件夹...', 'baidu_root')
  }

  static async aReLoadPikPakDrive(token: ITokenInfo): Promise<void> {
    await PanDAL.aReLoadProviderDrive(token, 'pikpak', token.default_drive_id || 'pikpak', '加载 PikPak 文件夹...', 'pikpak_root')
  }

  static async aReLoadQuarkDrive(token: ITokenInfo): Promise<void> {
    await PanDAL.aReLoadProviderDrive(token, 'quark', token.default_drive_id || 'quark', '加载夸克网盘文件夹...', 'quark_root')
  }

  static async aReLoadCloud139Drive(token: ITokenInfo): Promise<void> {
    await PanDAL.aReLoadProviderDrive(token, '139', token.default_drive_id || 'cloud139', '加载 139 云盘文件夹...', 'cloud139_root')
  }

  static async aReLoadCloud189Drive(token: ITokenInfo): Promise<void> {
    await PanDAL.aReLoadProviderDrive(token, '189', token.default_drive_id || 'cloud189', '加载天翼云盘文件夹...', 'cloud189_root')
  }

  static async aReLoadGuangyaDrive(token: ITokenInfo): Promise<void> {
    await PanDAL.aReLoadProviderDrive(token, 'guangya', token.default_drive_id || 'guangya', '加载光鸭云盘文件夹...', 'guangya_root')
  }

  static async aReLoadDropboxDrive(token: ITokenInfo): Promise<void> {
    await PanDAL.aReLoadProviderDrive(token, 'dropbox', token.default_drive_id || 'dropbox', '加载 Dropbox 文件夹...', 'dropbox_root')
  }

  static async aReLoadOneDrive(token: ITokenInfo): Promise<void> {
    await PanDAL.aReLoadProviderDrive(token, 'onedrive', token.default_drive_id || 'onedrive', '加载 OneDrive 文件夹...', 'onedrive_root')
  }

  static async aReLoadBoxDrive(token: ITokenInfo): Promise<void> {
    await PanDAL.aReLoadProviderDrive(token, 'box', token.default_drive_id || 'box', '加载 Box 文件夹...', 'box_root')
  }

  static async aReLoadGoogleDrive(token: ITokenInfo): Promise<void> {
    const driveId = token.default_drive_id || 'google'
    await PanDAL.aReLoadProviderDrive(token, 'google', driveId, '加载 Google Drive 文件夹...', 'google_root')
  }

  static async aReLoadBackupDrive(token: ITokenInfo): Promise<void> {
    const { user_id, default_drive_id, resource_drive_id, backup_drive_id, pic_drive_id } = token
    const drive_id = backup_drive_id
    const pantreeStore = usePanTreeStore()
    // 保存DriveId
    pantreeStore.mSaveUser(user_id, default_drive_id, resource_drive_id, backup_drive_id, pic_drive_id)
    pantreeStore.drive_id = drive_id
    if (!user_id || !pantreeStore.drive_id) return
    const backupCache = await DB.getValueObject('AllDir_' + drive_id)
    if (backupCache) {
      console.log('aReLoadDrive backupCache')
      await TreeStore.ConvertToOneDriver(user_id, drive_id, backupCache as IAliGetDirModel[], false, true)
    }
    if (backupCache) {
      const dt = await DB.getValueNumber('AllDir_' + drive_id)
      if (Date.now() - dt < 1000 * 60 * 60) {
        return
      }
    }
    beginAllDirLoading(drive_id)
    window.WinMsgToUpload({ cmd: 'AllDirList', user_id, drive_id: drive_id, drive_root: 'backup_root' })
  }

  static async aReLoadResourceDrive(token: ITokenInfo): Promise<void> {
    const { user_id, default_drive_id, resource_drive_id, backup_drive_id, pic_drive_id } = token
    const pantreeStore = usePanTreeStore()
    // 保存DriveId
    pantreeStore.mSaveUser(user_id, default_drive_id, resource_drive_id, backup_drive_id, pic_drive_id)
    if (!user_id || !resource_drive_id) return
    const resourceCache = await DB.getValueObject('AllDir_' + resource_drive_id)
    if (resourceCache) {
      console.log('aReLoadDrive resourceCache')
      await TreeStore.ConvertToOneDriver(user_id, resource_drive_id, resourceCache as IAliGetDirModel[], false, true)
    }
    if (resourceCache) {
      const dt = await DB.getValueNumber('AllDir_' + resource_drive_id)
      if (Date.now() - dt < 1000 * 60 * 60) {
        return
      }
    }
    beginAllDirLoading(resource_drive_id)
    window.WinMsgToUpload({ cmd: 'AllDirList', user_id, drive_id: resource_drive_id, drive_root: 'resource_root' })
  }

  static async aReLoadDriveSave(OneDriver: IDriverModel, error: string, drive_id: string): Promise<void> {
    try {
      if (error == 'time') {
        return
      } else if (!error) {
        await TreeStore.SaveOneDriver(OneDriver)
        PanDAL.RefreshPanTreeAllNode(OneDriver.drive_id)
      } else {
        message.error('列出全盘文件夹失败' + error)
      }
    } finally {
      finishAllDirLoading(drive_id || OneDriver?.drive_id || '')
    }
  }

  static aReLoadDriveProgress(drive_id: string, index: number, total: number): void {
    if (!AllDirLoadingDrives.has(drive_id)) return
    useFootStore().mSaveLoading('加载全部文件夹(' + Math.floor((index * 100) / (total + 1)) + '%)')
  }


  static RefreshPanTreeAllNode(drive_id: string) {
    const OneDriver = TreeStore.GetDriver(drive_id)
    if (!OneDriver) return
    const pantreeStore = usePanTreeStore()
    const driveType = GetDriveType(usePanTreeStore().user_id, drive_id)
    const dir: TreeNodeData = {
      __v_skip: true,
      key: driveType.key,
      drive_id: drive_id,
      parent_file_id: '',
      title: driveType.title,
      namesearch: '',
      children: []
    }
    const expandedKeys = new Set(usePanTreeStore().treeExpandedKeys)
    const map = new Map<string, TreeNodeData>()
    TreeStore.GetTreeDataToShow(OneDriver, dir, expandedKeys, map, true)
    map.set(dir.key, dir)
    const roots = [dir]
    if (resolveDriveProvider(pantreeStore.user_id, drive_id, UserDAL.GetUserToken(pantreeStore.user_id)?.tokenfrom).provider === 'google') {
      const shared: TreeNodeData = { __v_skip: true, key: 'google_shared', drive_id, parent_file_id: '', title: '共享云端硬盘', namesearch: '', children: [] }
      TreeStore.GetTreeDataToShow(OneDriver, shared, expandedKeys, map, true)
      map.set(shared.key, shared)
      roots.push(shared)
    }
    pantreeStore.mSaveTreeAllNode(OneDriver.drive_id, roots, map)
  }

  static GetPanTreeAllNode(user_id: string, drive_id: string, treeExpandedKeys: string[], getChildren: boolean = true, isLeafForce: boolean = false): TreeNodeData[] {
    const driveType = GetDriveType(user_id, drive_id)
    const dir: TreeNodeData = {
      __v_skip: true,
      title: driveType.title,
      drive_id: drive_id,
      parent_file_id: '',
      namesearch: '',
      key: driveType.key,
      children: []
    }
    const OneDriver = TreeStore.GetDriver(drive_id)
    if (!OneDriver) return [dir]
    const expandedKeys = new Set(treeExpandedKeys)
    const map = new Map<string, TreeNodeData>()
    TreeStore.GetTreeDataToShow(OneDriver, dir, expandedKeys, map, getChildren, '', isLeafForce)
    map.set(dir.key, dir)
    return [dir]
  }


  static aTreeScrollToDir(dirID: string) {
    usePanTreeStore().mSaveTreeScrollTo(dirID)
    usePanFileStore().mSaveFileScrollTo(dirID)
  }


  static async aReLoadOneDirToShow(drive_id: string, file_id: string, selfExpand: boolean, album_id: string = '', quickDirPath: IAliGetDirModel[] = []): Promise<boolean> {
    const panTreeStore = usePanTreeStore()
    const user_id = panTreeStore.user_id
    const driveType = GetDriveType(user_id, drive_id)
    const isBack = file_id == 'back'
    if (!drive_id) {
      drive_id = GetDriveID(user_id, file_id) || panTreeStore.drive_id
    }
    panTreeStore.drive_id = drive_id
    if (file_id == 'refresh') {
      file_id = panTreeStore.selectDir.file_id
    }
    if (isBack) {
      if (panTreeStore.History.length > 0) {
        panTreeStore.History.shift()
        if (panTreeStore.History.length > 0) {
          drive_id = panTreeStore.History[0].drive_id
          file_id = panTreeStore.History[0].file_id
        }
      }
      if (file_id == 'back') {
        file_id = driveType.key
        panTreeStore.History = []
      }
      if (file_id.includes('pic')) {
        panTreeStore.selectDir.album_type = file_id
      } else {
        panTreeStore.selectDir.album_type = 'pic_root'
        panTreeStore.selectDir.album_id = ''
      }
    }
    let dir = TreeStore.GetDir(drive_id, file_id)
    let dirPath = TreeStore.GetDirPath(drive_id, file_id)
    if ((!dir || dirPath.length === 0) && quickDirPath.length > 0) {
      dirPath = quickDirPath
      dir = { ...quickDirPath[quickDirPath.length - 1] }
    }
    const isCloudUser = isCloud123User(user_id)
    const providerRoute = resolveDriveProvider(user_id, drive_id, UserDAL.GetUserToken(user_id)?.tokenfrom)
    if (!dir || (dirPath.length == 0 && !file_id.includes('root'))) {
      if (isCloudUser && file_id !== 'cloud_root') {
        // 123 网盘不支持路径查询，依赖已加载的目录结构
      } else if (isBaiduUser(user_id) && file_id.startsWith('/')) {
        const dirName = file_id === '/' ? '根目录' : file_id.split('/').filter(Boolean).pop() || '根目录'
        dir = {
          __v_skip: true,
          file_id: file_id,
          drive_id: drive_id,
          parent_file_id: '',
          name: dirName,
          namesearch: '',
          description: '',
          time: 0,
          size: 0
        } as IAliGetDirModel
        dirPath = [dir]
      } else if (file_id.includes('root')) {
        const driveType = GetDriveType(user_id, drive_id)
        dir = {
          __v_skip: true,
          file_id: file_id,
          drive_id: drive_id,
          parent_file_id: '',
          name: driveType.title || '根目录',
          namesearch: '',
          description: '',
          time: 0,
          size: 0
        } as IAliGetDirModel
        dirPath = [dir]
      } else if (providerRoute.provider === 'google' && file_id === 'google_shared') {
        dir = {
          __v_skip: true,
          file_id,
          drive_id,
          parent_file_id: '',
          name: '共享云端硬盘',
          namesearch: '',
          description: '',
          time: 0,
          size: 0
        } as IAliGetDirModel
        dirPath = [dir]
      } else {
        if (!album_id && providerRoute.isValid && providerRoute.provider !== 'aliyun') {
          const providerDir = await getProviderFileInfo(providerRoute.provider, user_id, drive_id, file_id)
          if (providerDir?.isDir) {
            const root = GetDriveType(user_id, drive_id)
            const parentFileId = providerDir.parent_file_id === 'root' ? root.key : providerDir.parent_file_id
            dir = { ...providerDir, parent_file_id: parentFileId }
            dirPath = parentFileId === root.key
              ? [{ __v_skip: true, file_id: root.key, drive_id, parent_file_id: '', name: root.title, namesearch: '', description: '', time: 0, size: 0 } as IAliGetDirModel, dir]
              : [dir]
          }
        } else {
          const findPath = album_id
            ? await AliAlbum.ApiAlbumGetPath(panTreeStore.user_id, drive_id, album_id)
            : await AliFile.ApiFileGetPath(panTreeStore.user_id, drive_id, file_id)
          if (findPath.length > 0) {
            dirPath = findPath
            dir = { ...dirPath[dirPath.length - 1] }
          }
        }
      }
    }
    if (!dir || (dirPath.length == 0 && !file_id.includes('root'))) {
      message.error('出错，找不到指定的文件夹 ' + file_id)
      return false
    }
    // 记录跳转历史
    if (!isBack && panTreeStore.selectDir.file_id != dir.file_id) {
      const history: IAliGetDirModel[] = [dir]
      for (let i = 0, maxi = panTreeStore.History.length; i < maxi; i++) {
        history.push(panTreeStore.History[i])
        if (history.length >= 50) break
      }
      panTreeStore.History = history
    }
    // 展开列表节点
    const treeExpandedKeys = new Set(panTreeStore.treeExpandedKeys)
    for (let i = 0, maxi = dirPath.length - 1; i < maxi; i++) {
      treeExpandedKeys.add(dirPath[i].file_id)
    }
    if (selfExpand) {
      treeExpandedKeys.add(dir.file_id)
    }
    panTreeStore.mShowDir(dir, dirPath, [dir.file_id], Array.from(treeExpandedKeys))
    // console.warn('selectDir', panTreeStore.selectDir)
    PanDAL.RefreshPanTreeAllNode(drive_id)
    const panfileStore = usePanFileStore()
    if (panfileStore.ListLoading && panfileStore.DriveID == drive_id && panfileStore.DirID == dir.file_id) {
      return false
    }
    panfileStore.mSaveDirFileLoading(drive_id, dir.file_id, dir.name, dir.album_id)
    return PanDAL.GetDirFileList(panTreeStore.user_id, dir.drive_id, dir.file_id, dir.name, dir.album_id)
  }


  static GetDirFileList(user_id: string, drive_id: string, dirID: string, dirName: string, albumID: string = '', hasFiles: boolean = true): Promise<boolean> {
    return new Promise<boolean>((resolve) => {
      if (dirID == 'search') {
        if (hasFiles) {
          usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
        }
        resolve(true)
        return
      }

      if (isWebDavDrive(drive_id)) {
        const connectionId = getWebDavConnectionId(drive_id)
        const connection = getWebDavConnection(connectionId)
        if (!connection) {
          if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
          message.warning('WebDAV 连接不存在，请重新连接')
          resolve(false)
          return
        }
        const requestPath = dirID === '/' ? '/' : dirID
        listWebDavDirectory(connection, requestPath)
          .then(async (allItems) => {
            const items = hasFiles ? allItems : allItems.filter((item) => item.isDir)
            const dir = NewIAliFileResp(user_id, drive_id, dirID, dirName || (dirID === '/' ? connection.name : dirID.split('/').pop() || connection.name))
            dir.items = items
            dir.itemsKey = new Set(items.map((item) => item.file_id))
            dir.next_marker = ''
            dir.itemsTotal = items.length
            const panfileStore = usePanFileStore()
            panfileStore.mSaveDirFileLoadingPart(0, dir, dir.itemsTotal || 0)
            if (!TreeStore.GetDriver(drive_id)) {
              await TreeStore.ConvertToOneDriver(user_id, drive_id, [], false, true)
            }
            await TreeStore.SaveOneDirFileList(dir, hasFiles)
            if (hasFiles) {
              panfileStore.mSaveDirFileLoadingFinish(drive_id, dirID, dir.items, dir.itemsTotal || 0)
            }
            PanDAL.RefreshPanTreeAllNode(drive_id)
            resolve(true)
          })
          .catch((err: any) => {
            if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
            message.warning('列出 WebDAV 文件夹失败 ' + (err?.message || ''))
            resolve(false)
          })
        return
      }

      const providerRoute = resolveDriveProvider(user_id, drive_id, UserDAL.GetUserToken(user_id)?.tokenfrom)
      if (!providerRoute.isValid) {
        if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
        message.warning(`列出文件夹失败：${providerRoute.error}`)
        resolve(false)
        return
      }

      if (providerRoute.provider === 'cloud123') {
        listProviderItems('cloud123', user_id, drive_id, dirID, hasFiles)!
          .then(async (result) => {
            const order = TreeStore.GetDirOrder(drive_id, dirID).replace('ext ', 'updated_at ').split(' ')
            OrderDir(order[0], order[1], result.items)
            await PanDAL.SaveProviderDirFileList(user_id, drive_id, dirID, dirName, result.items, result.total, hasFiles)
            PanDAL.saveProviderCursor(user_id, drive_id, dirID, hasFiles, result.nextCursor)
            resolve(true)
          })
          .catch(() => {
            if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
            resolve(false)
          })
        return
      }

      if (providerRoute.provider === '115') {
        listProviderItems('115', user_id, drive_id, dirID, hasFiles)!
          .then(async (result) => {
            const order = TreeStore.GetDirOrder(drive_id, dirID).replace('ext ', 'updated_at ').split(' ')
            OrderDir(order[0], order[1], result.items)
            await PanDAL.SaveProviderDirFileList(user_id, drive_id, dirID, dirName, result.items, result.total, hasFiles)
            PanDAL.saveProviderCursor(user_id, drive_id, dirID, hasFiles, result.nextCursor)
            resolve(true)
          })
          .catch(() => {
            if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
            resolve(false)
          })
        return
      }

      if (providerRoute.provider === 'baidu') {
        listProviderItems('baidu', user_id, drive_id, dirID, hasFiles)!
          .then(async (result) => {
            const order = TreeStore.GetDirOrder(drive_id, dirID).replace('ext ', 'updated_at ').split(' ')
            OrderDir(order[0], order[1], result.items)
            await PanDAL.SaveProviderDirFileList(user_id, drive_id, dirID, dirName, result.items, result.total, hasFiles)
            PanDAL.saveProviderCursor(user_id, drive_id, dirID, hasFiles, result.nextCursor)
            resolve(true)
          })
          .catch(() => {
            if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
            resolve(false)
          })
        return
      }

      if (providerRoute.provider === 'pikpak') {
        listProviderItems('pikpak', user_id, drive_id, dirID, hasFiles)!
          .then(async (result) => {
            if (result.error) message.warning(result.error)
            const order = TreeStore.GetDirOrder(drive_id, dirID).replace('ext ', 'updated_at ').split(' ')
            OrderDir(order[0], order[1], result.items)
            await PanDAL.SaveProviderDirFileList(user_id, drive_id, dirID, dirName, result.items, result.total, hasFiles)
            PanDAL.saveProviderCursor(user_id, drive_id, dirID, hasFiles, result.nextCursor)
            resolve(true)
          })
          .catch(() => {
            if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
            resolve(false)
          })
        return
      }

      if (providerRoute.provider === 'quark') {
        listProviderItems('quark', user_id, drive_id, dirID, hasFiles)!
          .then(async (result) => {
            const order = TreeStore.GetDirOrder(drive_id, dirID).replace('ext ', 'updated_at ').split(' ')
            OrderDir(order[0], order[1], result.items)
            await PanDAL.SaveProviderDirFileList(user_id, drive_id, dirID, dirName, result.items, result.total, hasFiles)
            PanDAL.saveProviderCursor(user_id, drive_id, dirID, hasFiles, result.nextCursor)
            resolve(true)
          })
          .catch(() => {
            if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
            resolve(false)
          })
        return
      }

      if (providerRoute.provider === '139' || providerRoute.provider === '189') {
        listProviderItems(providerRoute.provider, user_id, drive_id, dirID, hasFiles)!
          .then(async (result) => {
            const order = TreeStore.GetDirOrder(drive_id, dirID).replace('ext ', 'updated_at ').split(' ')
            OrderDir(order[0], order[1], result.items)
            await PanDAL.SaveProviderDirFileList(user_id, drive_id, dirID, dirName, result.items, result.total, hasFiles)
            PanDAL.saveProviderCursor(user_id, drive_id, dirID, hasFiles, result.nextCursor)
            resolve(true)
          })
          .catch(() => {
            if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
            resolve(false)
          })
        return
      }

      if (providerRoute.provider === 'guangya') {
        listProviderItems('guangya', user_id, drive_id, dirID, hasFiles)!
          .then(async (result) => {
            const order = TreeStore.GetDirOrder(drive_id, dirID).replace('ext ', 'updated_at ').split(' ')
            OrderDir(order[0], order[1], result.items)
            await PanDAL.SaveProviderDirFileList(user_id, drive_id, dirID, dirName, result.items, result.total, hasFiles)
            PanDAL.saveProviderCursor(user_id, drive_id, dirID, hasFiles, result.nextCursor)
            resolve(true)
          })
          .catch(() => {
            if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
            resolve(false)
          })
        return
      }

      if (providerRoute.provider === 'dropbox') {
        listProviderItems('dropbox', user_id, drive_id, dirID, hasFiles)!
          .then(async (result) => {
            if (result.error) message.warning('搜索失败 ' + result.error)
            const order = TreeStore.GetDirOrder(drive_id, dirID).replace('ext ', 'updated_at ').split(' ')
            OrderDir(order[0], order[1], result.items)
            await PanDAL.SaveProviderDirFileList(user_id, drive_id, dirID, dirName, result.items, result.total, hasFiles)
            PanDAL.saveProviderCursor(user_id, drive_id, dirID, hasFiles, result.nextCursor)
            resolve(true)
          })
          .catch(() => {
            if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
            resolve(false)
          })
        return
      }

      if (providerRoute.provider === 'onedrive') {
        listProviderItems('onedrive', user_id, drive_id, dirID, hasFiles)!
          .then(async (result) => {
            if (result.error) message.warning('搜索失败 ' + result.error)
            const order = TreeStore.GetDirOrder(drive_id, dirID).replace('ext ', 'updated_at ').split(' ')
            OrderDir(order[0], order[1], result.items)
            await PanDAL.SaveProviderDirFileList(user_id, drive_id, dirID, dirName, result.items, result.total, hasFiles)
            PanDAL.saveProviderCursor(user_id, drive_id, dirID, hasFiles, result.nextCursor)
            resolve(true)
          })
          .catch(() => {
            if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
            resolve(false)
          })
        return
      }

      if (providerRoute.provider === 'google') {
        listProviderItems('google', user_id, drive_id, dirID, hasFiles)!
          .then((result) => {
            if (result.error) message.warning('搜索失败 ' + result.error)
            const order = TreeStore.GetDirOrder(drive_id, dirID).replace('ext ', 'updated_at ').split(' ')
            OrderDir(order[0], order[1], result.items)
            const dir = NewIAliFileResp(user_id, drive_id, dirID, dirName)
            dir.items = result.items
            dir.itemsKey = new Set(result.items.map(item => item.file_id))
            dir.itemsTotal = result.total
            const panfileStore = usePanFileStore()
            panfileStore.mSaveDirFileLoadingPart(0, dir, dir.itemsTotal || 0)
            TreeStore.SaveOneDirFileList(dir, hasFiles).then(() => {
              if (hasFiles) panfileStore.mSaveDirFileLoadingFinish(drive_id, dirID, dir.items, dir.itemsTotal || 0)
              PanDAL.saveProviderCursor(user_id, drive_id, dirID, hasFiles, result.nextCursor)
              PanDAL.RefreshPanTreeAllNode(drive_id)
              resolve(true)
            })
          })
          .catch(() => {
            if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
            resolve(false)
          })
        return
      }

      if (providerRoute.provider === 'box') {
        listProviderItems('box', user_id, drive_id, dirID, hasFiles)!
          .then(async (result) => {
            if (result.error) message.warning('搜索失败 ' + result.error)
            const order = TreeStore.GetDirOrder(drive_id, dirID).replace('ext ', 'updated_at ').split(' ')
            OrderDir(order[0], order[1], result.items)
            await PanDAL.SaveProviderDirFileList(user_id, drive_id, dirID, dirName, result.items, result.total, hasFiles)
            PanDAL.saveProviderCursor(user_id, drive_id, dirID, hasFiles, result.nextCursor)
            resolve(true)
          })
          .catch(() => {
            if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
            resolve(false)
          })
        return
      }

      const order = TreeStore.GetDirOrder(drive_id, dirID).replace('ext ', 'updated_at ')
      AliDirFileList.ApiDirFileList(user_id, drive_id, dirID, dirName, order, hasFiles ? '' : 'folder', albumID)
        .then((dir) => {
          if (!dir.next_marker) {
            dir.dirID = dirID // 修复root
            TreeStore.SaveOneDirFileList(dir, hasFiles).then(() => {
              if (hasFiles) {
                usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, dir.items, dir.itemsTotal || 0)
              }
              PanDAL.RefreshPanTreeAllNode(drive_id)
              resolve(true)
            })
          } else if (dir.next_marker == 'cancel') {
            resolve(false)
          } else {
            message.warning('列出文件夹失败 ' + dir.next_marker)
            if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
            resolve(false)
          }
        })
        .catch((err: any) => {
          if (hasFiles) usePanFileStore().mSaveDirFileLoadingFinish(drive_id, dirID, [])
          message.warning('列出文件夹失败 ' + (err.message || ''))
          DebugLog.mSaveWarning('列出文件夹失败file_id=' + dirID, err)
          resolve(false)
        })
    })
  }


  static aReLoadOneDirToRefreshTree(user_id: string, drive_id: string, dirID: string, albumID?: string): Promise<boolean> {
    return new Promise<boolean>((resolve) => {
      if (dirID == 'favorite' || dirID == 'recent' || dirID.startsWith('color')
        || dirID.startsWith('search') || dirID.startsWith('video')) {
        resolve(true)
        return
      }
      if (RefreshLock.has(dirID)) {
        resolve(true)
        return
      }
      RefreshLock.add(dirID)
      PanDAL.GetDirFileList(user_id, drive_id, dirID, '', albumID, false)
        .then((success) => {
          if (!success) {
            RefreshLock.delete(dirID)
            resolve(false)
            return
          }
          PanDAL.RefreshPanTreeAllNode(drive_id)
          const pantreeStore = usePanTreeStore()
          if (pantreeStore.selectDir.drive_id == drive_id && pantreeStore.selectDir.file_id == dirID) {
            PanDAL.aReLoadOneDirToShow(drive_id, dirID, false, albumID).then(() => {
              RefreshLock.delete(dirID)
              resolve(true)
            })
          } else {
            RefreshLock.delete(dirID)
            resolve(true)
          }
        })
        .catch((err: any) => {
          DebugLog.mSaveWarning('列出文件夹失败file_id=' + dirID, err)
          RefreshLock.delete(dirID)
          resolve(false)
        })
    })
  }

  static GetPanSelectedData(istree: boolean): PanSelectedData {
    const panTreeStore = usePanTreeStore()
    const panFileStore = usePanFileStore()
    const data: PanSelectedData = {
      isError: false,
      isErrorSelected: false,
      user_id: panTreeStore.user_id,
      drive_id: panTreeStore.drive_id,
      dirID: panTreeStore.selectDir.file_id,
      albumId: panTreeStore.selectDir.album_id || '',
      parentDirID: panTreeStore.selectDir.parent_file_id,
      selectedKeys: istree ? [panTreeStore.selectDir.file_id] : panFileStore.GetSelectedID(),
      selectedParentKeys: istree ? [panTreeStore.selectDir.parent_file_id] : panFileStore.GetSelectedParentDirID(),
      fileDescription: panFileStore.GetSelectedFirst()?.description || '',
      parentDirDescription: panTreeStore.selectDir.description
    }

    data.isError = !data.user_id || !data.drive_id || !data.dirID
    data.isErrorSelected = data.selectedKeys.length == 0
    return data
  }

  static updateQuickFile(list: QuickFileEntry[]) {
    if (list.length == 0) return
    const pantreeStore = usePanTreeStore()
    const arr = mergeQuickFiles(PanDAL.getQuickFileList(), list)
    localStorage.setItem(QUICK_FILE_STORAGE_KEY, JSON.stringify(arr))
    pantreeStore.mSaveQuick(arr)
  }


  static deleteQuickFile(id: string) {
    if (!id) return
    const pantreeStore = usePanTreeStore()
    const newArray = PanDAL.getQuickFileList().filter(item => item.id !== id)
    localStorage.setItem(QUICK_FILE_STORAGE_KEY, JSON.stringify(newArray))
    pantreeStore.mSaveQuick(newArray)
  }


  static getQuickFileList(): QuickFileEntry[] {
    try {
      const jsonstr = localStorage.getItem(QUICK_FILE_STORAGE_KEY)
      const list = jsonstr ? JSON.parse(jsonstr) : []
      return Array.isArray(list) ? list : []
    } catch {
      return []
    }
  }


  static aReLoadQuickFile(user_id: string) {
    let arr = PanDAL.getQuickFileList()
    const legacyKey = 'FileQuick-' + user_id
    const token = UserDAL.GetUserToken(user_id)
    const userName = token.nick_name || token.user_name || token.name || user_id
    try {
      const jsonstr = localStorage.getItem(legacyKey)
      const legacy = jsonstr ? JSON.parse(jsonstr) as LegacyQuickFileEntry[] : []
      if (Array.isArray(legacy) && legacy.length > 0) {
        for (const item of legacy) {
          const provider = GetDriveType(user_id, item.drive_id).name || ''
          arr = migrateLegacyQuickFiles(arr, user_id, [item], provider, userName)
        }
        localStorage.setItem(QUICK_FILE_STORAGE_KEY, JSON.stringify(arr))
        localStorage.removeItem(legacyKey)
      }
    } catch {
      // 保留损坏的旧数据，避免迁移时进一步丢失
    }
    usePanTreeStore().mSaveQuick(arr)
  }

  static async aOpenQuickFile(item: QuickFileEntry): Promise<boolean> {
    const dirPath = buildQuickFilePath(item).map(node => ({
      __v_skip: true,
      drive_id: node.drive_id || item.drive_id,
      file_id: node.file_id,
      parent_file_id: node.parent_file_id || '',
      name: node.name,
      namesearch: '',
      path: node.path,
      description: node.description || '',
      size: 0,
      time: 0
    } as IAliGetDirModel))
    return PanDAL.aReLoadOneDirToShow(item.drive_id, item.file_id, true, '', dirPath)
  }


  static async aUpdateDirFileSize(drive_id: string): Promise<void> {
    const pantreeStore = usePanTreeStore()
    const user_id = pantreeStore.user_id

    const diridList = TreeStore.GetDirSizeNeedRefresh(drive_id, 604800)
    const partList: string[] = []
    for (let i = 0, maxi = diridList.length; i < maxi; i++) {
      partList.push(diridList[i])
      if (partList.length >= 30) {
        const partResult = await AliDirFileList.ApiDirFileSize(user_id, drive_id, partList)
        if (!partResult) return
        if (partResult) TreeStore.SaveDirSizeNeedRefresh(drive_id, partResult)
        partList.length = 0
      }
    }
    if (partList.length > 0) {
      const partResult = await AliDirFileList.ApiDirFileSize(user_id, drive_id, partList)
      if (partResult) TreeStore.SaveDirSizeNeedRefresh(drive_id, partResult)
      partList.length = 0
    }
  }
}
