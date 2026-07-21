const getStorage = () => {
  try {
    return typeof window !== 'undefined' ? window.localStorage : undefined
  } catch {
    return undefined
  }
}

const normalizeVideoDriveId = (driveId: string) => {
  if (driveId === '115') return 'drive115'
  return driveId
}

export const videoProgressStorageKey = (userId: string, driveId: string, fileId: string) =>
  `boxplayer:video-progress:${userId}:${normalizeVideoDriveId(driveId)}:${fileId}`

export const getLocalVideoProgress = (userId: string, driveId: string, fileId: string): number => {
  const storage = getStorage()
  if (!storage || !userId || !driveId || !fileId) return 0
  const values = [
    storage.getItem(videoProgressStorageKey(userId, driveId, fileId)),
    // Read the pre-normalization key once so existing progress is preserved.
    storage.getItem(`boxplayer:video-progress:${userId}:${driveId}:${fileId}`),
    storage.getItem(`boxplayer:video-progress:shared:${userId}:${fileId}`)
  ].map(value => Number(value || 0))
  const progress = Math.max(...values.filter(value => Number.isFinite(value) && value > 0), 0)
  return Math.floor(progress)
}

export const saveLocalVideoProgress = (userId: string, driveId: string, fileId: string, position: number) => {
  const storage = getStorage()
  if (!storage || !userId || !driveId || !fileId || !Number.isFinite(position) || position <= 0) return
  try {
    const value = String(Math.floor(position))
    storage.setItem(videoProgressStorageKey(userId, driveId, fileId), value)
  } catch {
  }
}
