import AliFileCmd from '../../aliapi/filecmd'
import type { MediaAcquisitionRunView, MediaAcquisitionTarget } from '@shared/types/mediaAcquisition'
import { normalizeMediaAcquisitionPlatform, normalizeMediaAcquisitionRootFolder } from './capabilities'
import { addMediaAcquisitionEvent, completeMediaAcquisitionRun } from './client'
import { ensureMediaAcquisitionLeafFolder } from './organizer'
import { isMediaAcquisitionPrimaryVideoName } from './organizerPolicy'
import { inspectMediaAcquisitionRequestedEpisodeCoverage, listMediaAcquisitionDirectoryEntries, listMediaAcquisitionTargetFiles } from './targetSnapshot'

export interface MediaAcquisitionStagingTarget {
  folderId: string
  folderName: string
  target: MediaAcquisitionTarget
  isFinalTarget: boolean
}

export interface MediaAcquisitionStagingCleanupResult {
  removed: boolean
  remainingEntries: number
}

/** Mirrors mediary-scout's transferCandidate coverage gate at the provider side-effect boundary. */
export async function stopMediaAcquisitionTransferWhenCoverageMet(run: MediaAcquisitionRunView): Promise<boolean> {
  const coverage = await inspectMediaAcquisitionRequestedEpisodeCoverage(run.target)
  if (!coverage.covered) return false
  const summary = coverage.seasons.map(season => `S${String(season.seasonNumber).padStart(2, '0')} E${season.requestedEpisodes.join('、E')}`).join('；')
  const message = `目标缺集已在目录中存在，跳过网盘转存：${summary}`
  await addMediaAcquisitionEvent(run.id, 'info', 'transfer', message, { tool: 'transferCoverageGate', ...coverage })
  await completeMediaAcquisitionRun(run.id, message, { tool: 'transferCoverageGate', ...coverage })
  return true
}

export async function ensureMediaAcquisitionStagingTarget(target: MediaAcquisitionTarget, runId: string, candidateId: string): Promise<MediaAcquisitionStagingTarget> {
  if (target.mediaType === 'movie') {
    const leaf = await ensureMediaAcquisitionLeafFolder(target)
    return { folderId: leaf.id, folderName: leaf.path.split('/').filter(Boolean).at(-1) || target.title, target: { ...target, targetParentFileId: leaf.id }, isFinalTarget: true }
  }
  const platform = normalizeMediaAcquisitionPlatform(target.targetPlatform)
  const parentId = normalizeMediaAcquisitionRootFolder(platform, target.targetParentFileId)
  const folderName = `BoxPlayer-acquiring-${runId.slice(0, 8)}-${candidateId.slice(0, 8)}`
  const existing = (await listMediaAcquisitionDirectoryEntries(target, parentId)).find(entry => entry.isDir && entry.name === folderName)
  if (existing) return { folderId: existing.id, folderName, target: { ...target, targetParentFileId: existing.id }, isFinalTarget: false }

  const created = await AliFileCmd.ApiCreatNewForder(target.targetUserId, target.targetDriveId, parentId, folderName, '', 'refuse')
  if (!created.file_id) throw new Error(created.error || '无法创建媒体获取暂存目录')
  const folderId = platform === '115' && created.file_id === '0' ? 'drive115_root' : created.file_id
  return { folderId, folderName, target: { ...target, targetParentFileId: folderId }, isFinalTarget: false }
}

export async function cleanupEmptyMediaAcquisitionStagingTarget(target: MediaAcquisitionTarget, folderId: string): Promise<MediaAcquisitionStagingCleanupResult> {
  const entries = await listMediaAcquisitionDirectoryEntries(target, folderId)
  if (entries.length) return { removed: false, remainingEntries: entries.length }
  const removed = await AliFileCmd.ApiTrashBatch(target.targetUserId, target.targetDriveId, [folderId])
  return { removed: removed.includes(folderId), remainingEntries: 0 }
}

/** A staging directory is owned by exactly one run/candidate and can be discarded once that candidate is terminal. */
export async function discardMediaAcquisitionStagingTarget(target: MediaAcquisitionTarget, folderId: string): Promise<MediaAcquisitionStagingCleanupResult> {
  const entries = await listMediaAcquisitionDirectoryEntries(target, folderId)
  const removed = await AliFileCmd.ApiTrashBatch(target.targetUserId, target.targetDriveId, [folderId])
  return { removed: removed.includes(folderId), remainingEntries: entries.length }
}

/** A provider error is not authoritative when a valid media file already materialized. */
export async function hasMaterializedMediaAcquisitionContent(staging: MediaAcquisitionStagingTarget): Promise<boolean> {
  const files = await listMediaAcquisitionTargetFiles(staging.target)
  return files.some(file => isMediaAcquisitionPrimaryVideoName(file.name) && (file.size === undefined || file.size > 0))
}

export async function cleanupFailedMediaAcquisitionStagingTarget(run: MediaAcquisitionRunView, candidateId: string, staging: MediaAcquisitionStagingTarget): Promise<void> {
  if (staging.isFinalTarget) {
    await addMediaAcquisitionEvent(run.id, 'warning', 'transfer', `候选导入失败，最终目录 ${staging.folderName} 已保留，避免删除已有媒体。`, { tool: 'stagingCleanup', candidateId, folderId: staging.folderId, folderName: staging.folderName, isFinalTarget: true, reason: 'candidateFailure' })
    return
  }
  try {
    const result = await discardMediaAcquisitionStagingTarget(run.target, staging.folderId)
    if (result.removed) {
      await addMediaAcquisitionEvent(run.id, 'info', 'transfer', `候选导入失败，已回收暂存目录 ${staging.folderName}${result.remainingEntries ? `（含 ${result.remainingEntries} 项残留）` : ''}。`, { tool: 'stagingCleanup', candidateId, folderId: staging.folderId, folderName: staging.folderName, remainingEntries: result.remainingEntries, reason: 'candidateFailure' })
    } else if (result.remainingEntries) {
      await addMediaAcquisitionEvent(run.id, 'warning', 'transfer', `候选导入失败后暂存目录仍有 ${result.remainingEntries} 项内容，回收未完成。`, { tool: 'stagingCleanup', candidateId, folderId: staging.folderId, folderName: staging.folderName, remainingEntries: result.remainingEntries, reason: 'candidateFailure' })
    }
  } catch (error: any) {
    await addMediaAcquisitionEvent(run.id, 'warning', 'transfer', `候选失败后的暂存目录清理失败，已保留目录：${error?.message || '未知错误'}`, { tool: 'stagingCleanup', candidateId, folderId: staging.folderId, folderName: staging.folderName, reason: 'candidateFailure' })
  }
}

/** Provider success responses are not treated as landed evidence until the staging tree is read back. */
export async function rereadMediaAcquisitionStagingTarget(run: MediaAcquisitionRunView, candidateId: string, staging: MediaAcquisitionStagingTarget): Promise<void> {
  try {
    const files = await listMediaAcquisitionTargetFiles(staging.target)
    await addMediaAcquisitionEvent(run.id, 'info', 'verify', files.length ? `转存后即时核验：暂存目录已读取到 ${files.length} 个文件，等待完整入库核验。` : '转存后即时核验：暂存目录暂未出现文件，等待网盘同步。', {
      tool: 'stagingReread',
      candidateId,
      folderId: staging.folderId,
      folderName: staging.folderName,
      isFinalTarget: staging.isFinalTarget,
      fileCount: files.length,
      preview: files.slice(0, 12).map(file => ({ name: file.name, size: file.size, path: file.path }))
    })
  } catch (error: any) {
    await addMediaAcquisitionEvent(run.id, 'warning', 'verify', `转存后即时核验失败，后续将继续重试：${error?.message || '未知错误'}`, { tool: 'stagingReread', candidateId, folderId: staging.folderId, folderName: staging.folderName, isFinalTarget: staging.isFinalTarget })
  }
}
