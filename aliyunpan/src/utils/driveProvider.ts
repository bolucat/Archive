import type { ITokenInfo } from '../user/userstore'

export type DriveProvider = ITokenInfo['tokenfrom']

export interface DriveProviderMeta {
  key: DriveProvider
  label: string
  icon: string
}

const driveProviderMap: Record<DriveProvider, DriveProviderMeta> = {
  aliyun: {
    key: 'aliyun',
    label: '阿里云盘',
    icon: 'images/drive-icons/aliyun.svg'
  },
  cloud123: {
    key: 'cloud123',
    label: '123网盘',
    icon: 'images/drive-icons/cloud123.svg'
  },
  '115': {
    key: '115',
    label: '115网盘',
    icon: 'images/drive-icons/drive115.svg'
  },
  '139': {
    key: '139',
    label: '139云盘',
    icon: 'images/drive-icons/cloud139.svg'
  },
  '189': {
    key: '189',
    label: '天翼云盘',
    icon: 'images/drive-icons/cloud189.svg'
  },
  guangya: {
    key: 'guangya',
    label: '光鸭云盘',
    icon: 'images/drive-icons/guangya.svg'
  },
  baidu: {
    key: 'baidu',
    label: '百度网盘',
    icon: 'images/drive-icons/baidu.svg'
  },
  pikpak: {
    key: 'pikpak',
    label: 'PikPak',
    icon: 'images/drive-icons/pikpak.png'
  },
  quark: {
    key: 'quark',
    label: '夸克网盘',
    icon: 'images/drive-icons/quark.svg'
  },
  dropbox: {
    key: 'dropbox',
    label: 'Dropbox',
    icon: 'images/drive-icons/dropbox.svg'
  },
  onedrive: {
    key: 'onedrive',
    label: 'OneDrive',
    icon: 'images/drive-icons/onedrive.svg'
  },
  box: {
    key: 'box',
    label: 'Box',
    icon: 'images/drive-icons/box.svg'
  },
  webdav: {
    key: 'webdav',
    label: 'WebDAV',
    icon: ''
  },
  alist: {
    key: 'alist',
    label: 'AList',
    icon: ''
  },
  unknown: {
    key: 'unknown',
    label: '未知网盘',
    icon: ''
  }
}

export const getDriveProviderMeta = (tokenfrom?: string): DriveProviderMeta => {
  return driveProviderMap[(tokenfrom || 'unknown') as DriveProvider] || driveProviderMap.unknown
}

export const getDriveProviderLabel = (tokenfrom?: string): string => getDriveProviderMeta(tokenfrom).label

export const getDriveProviderIcon = (tokenfrom?: string): string => getDriveProviderMeta(tokenfrom).icon
