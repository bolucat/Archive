import type { MediaAcquisitionTarget } from '@shared/types/mediaAcquisition'
import { normalizeMediaAcquisitionPlatform } from './capabilities'
import { submitMediaAcquisitionExternalUrl } from './client'

export interface ExternalUrlOfflineSubmission {
  message: string
  platform: string
  taskId?: string
  fileId?: string
}

export async function submitExternalUrlOffline(runId: string, target: MediaAcquisitionTarget, url: string, fileName: string): Promise<ExternalUrlOfflineSubmission> {
  const platform = normalizeMediaAcquisitionPlatform(target.targetPlatform)
  const result = await submitMediaAcquisitionExternalUrl(runId, target.targetParentFileId, url, fileName)
  return { message: result.activity, platform, taskId: result.taskId, fileId: result.fileId }
}
