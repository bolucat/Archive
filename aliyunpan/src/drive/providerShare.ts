import { apiBoxShareRemove, apiBoxShareUpdate } from '../box/share'
import { cloud123ShareUpdateNotice } from '../cloud123/share'
import { listBoxShares } from '../box/adapter'
import { listCloud123Shares } from '../cloud123/adapter'
import { listDropboxShares } from '../dropbox/adapter'
import { listGuangyaShares } from '../guangya/adapter'
import { listPikPakShares } from '../pikpak/adapter'
import { listQuarkShares } from '../quark/adapter'
import { apiGuangyaSaveShareFilesBatch, apiGuangyaShareAnonymous, apiGuangyaShareDelete, apiGuangyaShareFileList, apiGuangyaShareToken, apiGuangyaShareUpdate, isGuangyaShareId } from '../guangya/share'
import { apiPikPakSaveShareFilesBatch, apiPikPakShareAnonymous, apiPikPakShareDelete, apiPikPakShareFileList, apiPikPakShareToken, apiPikPakShareUpdate, isPikPakShareId } from '../pikpak/share'
import { apiQuarkSaveShareFilesBatch, apiQuarkShareAnonymous, apiQuarkShareCancelBatch, apiQuarkShareFileList, apiQuarkShareToken, apiQuarkShareUpdateBatch, decodeQuarkShareId, isQuarkShareId } from '../quark/share'
import type { DriveProvider } from '../utils/driveProvider'
import UserDAL from '../user/userdal'
import { resolveDriveProvider } from '../utils/driveProvider'

export const listProviderAccountShares = async (userId: string) => {
  const route = resolveDriveProvider(userId, '', UserDAL.GetUserToken(userId)?.tokenfrom)
  switch (route.provider) {
    case 'box': return { items: await listBoxShares(userId), error: '' }
    case 'cloud123': return await listCloud123Shares(userId)
    case 'dropbox': return await listDropboxShares(userId)
    case 'quark': return { items: await listQuarkShares(userId), error: '' }
    case 'guangya': return await listGuangyaShares(userId)
    case 'pikpak': return await listPikPakShares(userId)
    default: return undefined
  }
}

export const getSharedProvider = (shareId: string): DriveProvider | 'unknown' => isQuarkShareId(shareId) ? 'quark' : isGuangyaShareId(shareId) ? 'guangya' : isPikPakShareId(shareId) ? 'pikpak' : 'unknown'

export const getProviderShareAnonymous = (shareId: string, sharePwd: string) => {
  switch (getSharedProvider(shareId)) {
    case 'quark': return apiQuarkShareAnonymous(shareId, sharePwd)
    case 'guangya': return apiGuangyaShareAnonymous(shareId, sharePwd)
    case 'pikpak': return apiPikPakShareAnonymous(shareId, sharePwd)
    default: return undefined
  }
}

export const getProviderShareToken = (shareId: string, sharePwd: string) => {
  switch (getSharedProvider(shareId)) {
    case 'quark': return apiQuarkShareToken(decodeQuarkShareId(shareId), sharePwd)
    case 'guangya': return apiGuangyaShareToken(shareId, sharePwd)
    case 'pikpak': return apiPikPakShareToken(shareId, sharePwd)
    default: return undefined
  }
}

export const getProviderShareFiles = (shareId: string, shareToken: string, parentFileId: string) => {
  switch (getSharedProvider(shareId)) {
    case 'quark': return apiQuarkShareFileList(shareId, shareToken, parentFileId)
    case 'guangya': return apiGuangyaShareFileList(shareId, shareToken, parentFileId)
    case 'pikpak': return apiPikPakShareFileList(shareId, shareToken, parentFileId)
    default: return undefined
  }
}

export const cancelProviderShares = async (provider: DriveProvider, userId: string, shareIds: string[]): Promise<string[] | undefined> => {
  switch (provider) {
    case 'quark': return apiQuarkShareCancelBatch(userId, shareIds)
    case 'guangya': return apiGuangyaShareDelete(userId, shareIds)
    case 'pikpak': return apiPikPakShareDelete(userId, shareIds)
    case 'box': {
      const success: string[] = []
      for (const shareId of shareIds) if (await apiBoxShareRemove(userId, shareId)) success.push(shareId)
      return success
    }
    default: return undefined
  }
}

export const updateProviderShares = async (provider: DriveProvider, userId: string, shareIds: string[], expirations: string[], passwords: string[], names?: string[]) => {
  switch (provider) {
    case 'box': {
      const success: { share_id: string; share_pwd: string; expiration: string; share_name: string }[] = []
      for (let index = 0; index < shareIds.length; index++) {
        const item = await apiBoxShareUpdate(userId, shareIds[index], expirations[index] || '', passwords[index] || '')
        if (item?.shared_link) success.push({ share_id: shareIds[index], share_pwd: item.shared_link.password || '', expiration: item.shared_link.unshared_at || '', share_name: item.name || names?.[index] || '' })
      }
      return success
    }
    case 'pikpak': {
      const success: { share_id: string; share_pwd: string; expiration: string; share_name: string }[] = []
      for (let index = 0; index < shareIds.length; index++) {
        const item = await apiPikPakShareUpdate(userId, shareIds[index], names?.[index] || '', expirations[index] || '', passwords[index] || '')
        if (item.success) success.push({ share_id: shareIds[index], share_pwd: passwords[index] || '', expiration: expirations[index] || '', share_name: names?.[index] || '' })
      }
      return success
    }
    case 'guangya': {
      const success: { share_id: string; share_pwd: string; expiration: string; share_name: string }[] = []
      for (let index = 0; index < shareIds.length; index++) {
        const item = await apiGuangyaShareUpdate(userId, shareIds[index], expirations[index] || '', passwords[index] || '', names?.[index] || '')
        if (item.success) success.push({ share_id: shareIds[index], share_pwd: passwords[index] || '', expiration: expirations[index] || '', share_name: names?.[index] || '' })
      }
      return success
    }
    case 'quark': return apiQuarkShareUpdateBatch(userId, shareIds, expirations, passwords, names)
    default: return undefined
  }
}

export const getProviderShareUpdateNotice = (provider: DriveProvider): string => provider === 'cloud123' ? cloud123ShareUpdateNotice : ''

export const saveProviderShareFiles = (provider: DriveProvider, shareId: string, shareToken: string, userId: string, parentFileId: string, fileIds: string[]) => {
  switch (provider) {
    case 'quark': return apiQuarkSaveShareFilesBatch(shareId, shareToken, userId, parentFileId, fileIds)
    case 'guangya': return apiGuangyaSaveShareFilesBatch(shareId, shareToken, userId, parentFileId, fileIds)
    case 'pikpak': return apiPikPakSaveShareFilesBatch(shareId, shareToken, userId, parentFileId, fileIds)
    default: return undefined
  }
}
