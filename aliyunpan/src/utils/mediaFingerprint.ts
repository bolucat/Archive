import type { DriveFileItem } from '../types/media'

export interface MediaFingerprint {
  fingerprintNamespace: string
  fingerprint: string
  fileSize: number
}

export function buildMediaFingerprint(file: Pick<DriveFileItem, 'driveServerId' | 'driveId' | 'fileSize' | 'contentHash' | 'contentHashName'>): MediaFingerprint | undefined {
  const fingerprint = String(file.contentHash || '').trim()
  const algorithm = String(file.contentHashName || '').trim().toLowerCase().replace(/[^a-z0-9_-]/g, '')
  const provider = String(file.driveServerId || file.driveId || '').trim().toLowerCase().replace(/[^a-z0-9_-]/g, '')
  const fileSize = Number(file.fileSize)
  if (!fingerprint || !algorithm || !provider || !Number.isSafeInteger(fileSize) || fileSize <= 0) return undefined
  return {
    fingerprintNamespace: `${provider}:${algorithm}`,
    fingerprint: /^[0-9a-f]+$/i.test(fingerprint) ? fingerprint.toLowerCase() : fingerprint,
    fileSize
  }
}
