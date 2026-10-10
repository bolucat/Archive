export const MEDIA_SERVER_DRIVE_ID = 'media_server'

export interface ProxyRefreshState {
  proxyKind?: string
  driveId: string
  fileId: string
  proxyUrl: string
  selectQuality: string
  proxyInfo?: {
    file_id?: string
    expires_time?: number
    videoQuality?: string
  }
}

export const shouldRefreshProxyUrl = (state: ProxyRefreshState): boolean => {
  // Subtitle addresses belong to their own file/track, never to the cached video.
  if (state.proxyKind === 'subtitle') return false
  if (state.driveId === MEDIA_SERVER_DRIVE_ID && state.proxyUrl) return false
  const needRefreshUrl = state.proxyInfo && (state.fileId !== state.proxyInfo.file_id || (state.proxyInfo.expires_time || 0) <= Date.now())
  const changeVideoQuality = state.proxyInfo && state.proxyInfo.videoQuality && state.selectQuality !== state.proxyInfo.videoQuality
  return !state.proxyUrl || !!needRefreshUrl || !!changeVideoQuality
}
