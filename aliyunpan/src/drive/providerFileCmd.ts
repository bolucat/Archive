import type { IAliGetFileModel } from '../aliapi/alimodels'
import { copyCloud123Files, createCloud123Folder, getCloud123FilesInfo, moveCloud123Files, renameCloud123Files, restoreCloud123Files, trashCloud123Files } from '../cloud123/adapter'
import { copyDrive115Files, createDrive115Folder, deleteDrive115TrashFiles, moveDrive115Files, renameDrive115Files, restoreDrive115TrashFiles, trashDrive115Files } from '../cloud115/adapter'
import { copyBaiduFiles, createBaiduFolder, moveBaiduFiles, renameBaiduFiles, trashBaiduFiles } from '../cloudbaidu/adapter'
import { copyPikPakFiles, createPikPakFolder, deletePikPakTrashFiles, movePikPakFiles, renamePikPakFiles, restorePikPakTrashFiles, trashPikPakFiles } from '../pikpak/adapter'
import { createQuarkFolder, moveQuarkFiles, renameQuarkFiles, trashQuarkFiles } from '../quark/adapter'
import { copyCloud139Files, createCloud139Folder, moveCloud139Files, renameCloud139Files, trashCloud139Files } from '../cloud139/adapter'
import { copyCloud189Files, createCloud189Folder, moveCloud189Files, renameCloud189Files, trashCloud189Files } from '../cloud189/adapter'
import { copyGuangyaFiles, createGuangyaFolder, moveGuangyaFiles, renameGuangyaFiles, trashGuangyaFiles } from '../guangya/adapter'
import { copyDropboxFiles, createDropboxFolder, moveDropboxFiles, renameDropboxFiles, trashDropboxFiles } from '../dropbox/adapter'
import { copyOneDriveFiles, createOneDriveFolder, moveOneDriveFiles, renameOneDriveFiles, trashOneDriveFiles } from '../onedrive/adapter'
import { cleanBoxTrashFiles, copyBoxFiles, createBoxFolder, moveBoxFiles, renameBoxFiles, restoreBoxTrashFiles, trashBoxFiles } from '../box/adapter'
import { copyGoogleFiles, createGoogleFolder, deleteGoogleFiles, moveGoogleFiles, renameGoogleFiles, restoreGoogleTrashFiles, trashGoogleFiles } from '../google/adapter'
import type { DriveProvider } from '../utils/driveProvider'
import usePanFileStore from '../pan/panfilestore'
import usePanTreeStore from '../pan/pantreestore'
import { resolveBaiduPaths, resolveBaiduTargetPath } from '../cloudbaidu/filecmd'
import { getBoxSelectedTypes } from '../box/filecmd'

export type ProviderRenameResult = { file_id: string; parent_file_id: string; name: string; isDir: boolean }
export type ProviderFileCommandOptions = { parentDescription?: string; names?: string[] | Map<string, string>; checkNameMode?: string; sourcePaths?: string[]; targetPath?: string; boxTypes?: Record<string, any> }
export type ProviderFileCommandNotice = 'delete' | 'trashClean' | 'trashRestore' | 'copy'

export const getProviderFileCommandNotice = (provider: DriveProvider, operation: ProviderFileCommandNotice): string => {
  if ((operation === 'delete' || operation === 'trashClean') && provider === 'cloud123') return '123 网盘开放 API 暂不支持彻底删除回收站文件，请在官方客户端或网页端操作'
  if (operation === 'delete' && provider === '115') return '115网盘不支持直接彻底删除，请先移入回收站后再删除'
  if (operation === 'delete' && provider === 'box') return 'Box 文件需先移入回收站后才能彻底删除'
  if (operation === 'trashClean' && provider === 'guangya') return '光鸭云盘请在官方客户端彻底删除回收站文件'
  if (operation === 'trashClean' && provider === 'dropbox') return 'Dropbox 已删除文件请在官方客户端恢复或彻底删除'
  if (operation === 'trashRestore' && provider === 'quark') return '夸克网盘请在官方客户端恢复回收站文件'
  if (operation === 'trashRestore' && provider === 'dropbox') return 'Dropbox 已删除文件请在官方客户端恢复'
  if (operation === 'trashRestore' && (provider === '139' || provider === '189' || provider === 'guangya')) return '请在官方客户端恢复回收站文件'
  if (operation === 'copy' && provider === 'quark') return '夸克网盘暂不支持复制'
  return ''
}

export const getProviderFileCommandContext = (provider: DriveProvider, fileIds: string[], targetFileId = '', targetDescription = ''): ProviderFileCommandOptions => {
  const fileStore = usePanFileStore()
  const selected = fileStore.GetSelected()
  if (provider === 'baidu') {
    return {
      sourcePaths: resolveBaiduPaths(fileIds, fileStore.ListDataRaw || []),
      targetPath: resolveBaiduTargetPath(targetFileId, targetFileId, targetDescription, fileStore.ListDataRaw || [], usePanTreeStore().selectDir)
    }
  }
  if (provider === 'box') return { boxTypes: getBoxSelectedTypes(fileIds, selected) }
  return {}
}

export const getProviderFolderCommandContext = (provider: DriveProvider, parentFileId: string): ProviderFileCommandOptions => {
  const fileStore = usePanFileStore()
  if (provider === 'dropbox') {
    const selected = usePanTreeStore().selectDir
    const parent = (fileStore.ListDataRaw || []).find(item => item.file_id === parentFileId)
    return { parentDescription: parent?.description || (selected.file_id === parentFileId ? selected.description : '') }
  }
  if (provider !== 'baidu') return {}
  return {
    targetPath: resolveBaiduTargetPath(parentFileId, '', '', fileStore.ListDataRaw || [], usePanTreeStore().selectDir)
  }
}

/** Thin routing table. Provider API calls stay in each provider adapter. */
export const createProviderFolder = (provider: DriveProvider, userId: string, parentFileId: string, name: string, options: ProviderFileCommandOptions = {}) => {
  switch (provider) {
    case 'cloud123': return createCloud123Folder(userId, parentFileId, name)
    case '115': return createDrive115Folder(userId, parentFileId, name)
    case 'baidu': {
      const parentPath = options.targetPath || (parentFileId.startsWith('/') ? parentFileId : '')
      if (!parentPath) return Promise.resolve({ file_id: '', error: '无法确定百度网盘父文件夹路径，请刷新目录后重试' })
      return createBaiduFolder(userId, parentPath, name, options.checkNameMode || 'refuse')
    }
    case 'pikpak': return createPikPakFolder(userId, parentFileId, name)
    case 'quark': return createQuarkFolder(userId, parentFileId, name)
    case '139': return createCloud139Folder(userId, parentFileId, name)
    case '189': return createCloud189Folder(userId, parentFileId, name)
    case 'guangya': return createGuangyaFolder(userId, parentFileId, name)
    case 'dropbox': return createDropboxFolder(userId, parentFileId, name, options.parentDescription)
    case 'onedrive': return createOneDriveFolder(userId, parentFileId, name)
    case 'box': return createBoxFolder(userId, parentFileId, name)
    case 'google': return createGoogleFolder(userId, parentFileId, name)
    default: return Promise.resolve({ file_id: '', error: '当前网盘不支持新建文件夹' })
  }
}

export const trashProviderFiles = (provider: DriveProvider, userId: string, fileIds: string[], options: ProviderFileCommandOptions = {}): Promise<string[]> => {
  switch (provider) {
    case 'cloud123': return trashCloud123Files(userId, fileIds)
    case '115': return trashDrive115Files(userId, fileIds)
    case 'baidu': return trashBaiduFiles(userId, options.sourcePaths || fileIds)
    case 'pikpak': return trashPikPakFiles(userId, fileIds)
    case 'quark': return trashQuarkFiles(userId, fileIds)
    case '139': return trashCloud139Files(userId, fileIds)
    case '189': return trashCloud189Files(userId, fileIds)
    case 'guangya': return trashGuangyaFiles(userId, fileIds)
    case 'dropbox': return trashDropboxFiles(userId, fileIds)
    case 'onedrive': return trashOneDriveFiles(userId, fileIds)
    case 'box': return trashBoxFiles(userId, fileIds)
    case 'google': return trashGoogleFiles(userId, fileIds)
    default: return Promise.resolve([])
  }
}

export const deleteProviderFiles = (provider: DriveProvider, userId: string, fileIds: string[], options: ProviderFileCommandOptions = {}): Promise<string[]> => {
  switch (provider) {
    case 'baidu': return trashBaiduFiles(userId, options.sourcePaths || fileIds)
    case 'pikpak': return deletePikPakTrashFiles(userId, fileIds)
    case 'quark': return trashQuarkFiles(userId, fileIds)
    case '139': return trashCloud139Files(userId, fileIds)
    case '189': return trashCloud189Files(userId, fileIds)
    case 'guangya': return trashGuangyaFiles(userId, fileIds)
    case 'dropbox': return trashDropboxFiles(userId, fileIds)
    case 'onedrive': return trashOneDriveFiles(userId, fileIds)
    case 'google': return deleteGoogleFiles(userId, fileIds)
    default: return Promise.resolve([])
  }
}

export const renameProviderFiles = (provider: DriveProvider, userId: string, fileIds: string[], names: string[], options: ProviderFileCommandOptions = {}): Promise<ProviderRenameResult[]> => {
  switch (provider) {
    case 'cloud123': return renameCloud123Files(userId, fileIds, names)
    case '115': return renameDrive115Files(userId, fileIds, names)
    case 'baidu': return renameBaiduFiles(userId, fileIds, options.sourcePaths || fileIds, names)
    case 'pikpak': return renamePikPakFiles(userId, fileIds, names)
    case 'quark': return renameQuarkFiles(userId, fileIds, names)
    case '139': return renameCloud139Files(userId, fileIds, names)
    case '189': return renameCloud189Files(userId, fileIds, names)
    case 'guangya': return renameGuangyaFiles(userId, fileIds, names)
    case 'dropbox': return renameDropboxFiles(userId, fileIds, names)
    case 'onedrive': return renameOneDriveFiles(userId, fileIds, names)
    case 'box': return renameBoxFiles(userId, fileIds, names)
    case 'google': return renameGoogleFiles(userId, fileIds, names)
    default: return Promise.resolve([])
  }
}

export const moveProviderFiles = (provider: DriveProvider, userId: string, fileIds: string[], parentFileId: string, options: ProviderFileCommandOptions = {}): Promise<string[]> => {
  switch (provider) {
    case 'cloud123': return moveCloud123Files(userId, fileIds, parentFileId)
    case '115': return moveDrive115Files(userId, fileIds, parentFileId)
    case 'baidu': {
      const targetPath = options.targetPath || (parentFileId.startsWith('/') ? parentFileId : '')
      return targetPath ? moveBaiduFiles(userId, options.sourcePaths || fileIds, targetPath) : Promise.resolve([])
    }
    case 'pikpak': return movePikPakFiles(userId, fileIds, parentFileId)
    case 'quark': return moveQuarkFiles(userId, fileIds, parentFileId)
    case '139': return moveCloud139Files(userId, fileIds, parentFileId)
    case '189': return moveCloud189Files(userId, fileIds, parentFileId)
    case 'guangya': return moveGuangyaFiles(userId, fileIds, parentFileId)
    case 'dropbox': return moveDropboxFiles(userId, fileIds, parentFileId, options.parentDescription)
    case 'onedrive': return moveOneDriveFiles(userId, fileIds, parentFileId)
    case 'box': return moveBoxFiles(userId, fileIds, parentFileId, options.boxTypes)
    case 'google': return moveGoogleFiles(userId, fileIds, parentFileId)
    default: return Promise.resolve([])
  }
}

export const copyProviderFiles = (provider: DriveProvider, userId: string, fileIds: string[], parentFileId: string, options: ProviderFileCommandOptions = {}): Promise<string[]> => {
  switch (provider) {
    case 'cloud123': return copyCloud123Files(userId, fileIds, parentFileId)
    case '115': return copyDrive115Files(userId, fileIds, parentFileId)
    case 'baidu': {
      const targetPath = options.targetPath || (parentFileId.startsWith('/') ? parentFileId : '')
      return targetPath ? copyBaiduFiles(userId, options.sourcePaths || fileIds, targetPath) : Promise.resolve([])
    }
    case 'pikpak': return copyPikPakFiles(userId, fileIds, parentFileId)
    case '139': return copyCloud139Files(userId, fileIds, parentFileId)
    case '189': return copyCloud189Files(userId, fileIds, parentFileId)
    case 'guangya': return copyGuangyaFiles(userId, fileIds, parentFileId)
    case 'dropbox': return copyDropboxFiles(userId, fileIds, parentFileId, options.parentDescription)
    case 'onedrive': {
      const names = options.names
      return copyOneDriveFiles(userId, fileIds, parentFileId, names instanceof Map ? fileIds.map(fileId => names.get(fileId) || '') : names || [])
    }
    case 'box': return copyBoxFiles(userId, fileIds, parentFileId, options.boxTypes)
    case 'google': return copyGoogleFiles(userId, fileIds, parentFileId)
    default: return Promise.resolve([])
  }
}

export const getProviderFilesInfo = async (provider: DriveProvider, userId: string, fileIds: string[]): Promise<IAliGetFileModel[]> => provider === 'cloud123' ? await getCloud123FilesInfo(userId, fileIds) as IAliGetFileModel[] : []

export const cleanProviderTrash = (provider: DriveProvider, userId: string, fileIds: string[], options: ProviderFileCommandOptions = {}): Promise<string[]> => {
  switch (provider) {
    case 'box': return cleanBoxTrashFiles(userId, fileIds, options.boxTypes)
    case 'google': return deleteGoogleFiles(userId, fileIds)
    case '115': return deleteDrive115TrashFiles(userId, fileIds)
    case 'pikpak': return deletePikPakTrashFiles(userId, fileIds)
    case 'quark': return trashQuarkFiles(userId, fileIds)
    default: return Promise.resolve([])
  }
}

export const restoreProviderTrash = (provider: DriveProvider, userId: string, fileIds: string[], options: ProviderFileCommandOptions = {}): Promise<string[]> => {
  switch (provider) {
    case 'box': return restoreBoxTrashFiles(userId, fileIds, options.boxTypes)
    case 'google': return restoreGoogleTrashFiles(userId, fileIds)
    case 'cloud123': return restoreCloud123Files(userId, fileIds)
    case '115': return restoreDrive115TrashFiles(userId, fileIds)
    case 'pikpak': return restorePikPakTrashFiles(userId, fileIds)
    default: return Promise.resolve([])
  }
}
