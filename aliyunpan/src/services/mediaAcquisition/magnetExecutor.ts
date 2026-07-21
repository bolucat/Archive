import { apiDrive115OfflineCreate } from '../../cloud115/offline'
import { apiCloud123OfflineCreate } from '../../cloud123/offline'
import { apiGuangyaOfflineCreate } from '../../guangya/offline'
import { apiPikPakOfflineCreate } from '../../pikpak/offline'
import type { MediaAcquisitionRunView } from '@shared/types/mediaAcquisition'
import { addMediaAcquisitionEvent, failMediaAcquisitionCandidate, getMediaAcquisitionCandidateLocator, markMediaAcquisitionCandidateTransferring, recordMediaAcquisitionCandidateBaseline, recordMediaAcquisitionExternalTask, recordMediaAcquisitionTransferIntent } from './client'
import { normalizeMediaAcquisitionPlatform, normalizeMediaAcquisitionRootFolder } from './capabilities'
import { canTryNextMediaAcquisitionCandidate } from './candidatePolicy'
import { listMediaAcquisitionTransferBaselineFiles } from './targetSnapshot'
import { cleanupFailedMediaAcquisitionStagingTarget, ensureMediaAcquisitionStagingTarget, hasMaterializedMediaAcquisitionContent, rereadMediaAcquisitionStagingTarget, stopMediaAcquisitionTransferWhenCoverageMet, type MediaAcquisitionStagingTarget } from './staging'
import DownDAL from '../../down/DownDAL'

export async function executeMediaAcquisitionMagnetCandidate(run: MediaAcquisitionRunView, candidateId: string): Promise<void> {
  const candidate = run.candidates.find(item => item.id === candidateId)
  if (!candidate) throw new Error('未找到候选资源')
  if (candidate.kind !== 'magnet') throw new Error('当前候选资源不是磁力链接')
  const platform = normalizeMediaAcquisitionPlatform(run.target.targetPlatform)
  if (!['115', 'guangya', 'pikpak'].includes(platform)) throw new Error(`${run.target.targetPlatform} 当前不支持磁力离线下载`)
  const source = await getMediaAcquisitionCandidateLocator(run.id, candidateId)
  if (!source?.locator.startsWith('magnet:?')) throw new Error('无效的磁力链接')

  let staging: MediaAcquisitionStagingTarget | undefined
  let externalTaskCreated = false
  let step = '读取入库目录快照'
  try {
    const claimed = await markMediaAcquisitionCandidateTransferring(run.id, candidateId)
    if (!claimed) return
    if (await stopMediaAcquisitionTransferWhenCoverageMet(run)) return
    await addMediaAcquisitionEvent(run.id, 'info', 'transfer', `开始${platformName(platform)}磁力离线诊断。`, { tool: 'magnetOffline', candidateId, platform, targetDriveId: run.target.targetDriveId })
    const baseline = await listMediaAcquisitionTransferBaselineFiles(run.target)
    step = '创建暂存目录'
    staging = await ensureMediaAcquisitionStagingTarget(run.target, run.id, candidateId)
    const parentId = normalizeMediaAcquisitionRootFolder(platform, staging.target.targetParentFileId)
    await recordMediaAcquisitionCandidateBaseline(run.id, candidateId, baseline)
    await addMediaAcquisitionEvent(run.id, 'info', 'transfer', staging.isFinalTarget ? `电影将直接导入最终目录 ${staging.folderName}，准备创建${platformName(platform)}离线下载任务。` : `已创建任务暂存目录 ${staging.folderName}，准备创建${platformName(platform)}离线下载任务。`, { candidateId, platform, title: candidate.title, parentId, folderId: staging.folderId, folderName: staging.folderName, isFinalTarget: staging.isFinalTarget, tool: 'staging' })
    step = '提交离线下载任务'
    await recordMediaAcquisitionTransferIntent(run.id, candidateId, '已记录磁力离线提交意图，正在请求网盘创建任务。')
    if (platform === '115') {
      const result = await apiDrive115OfflineCreate(run.target.targetUserId, source.locator, parentId)
      if (result.error) throw new Error(result.error)
      DownDAL.aTrackDrive115OfflineDownload(run.target.targetUserId, source.locator, parentId, result.taskIds, candidate.title)
      externalTaskCreated = true
      step = '回读暂存目录'
      await recordMediaAcquisitionExternalTask(run.id, candidateId, result.taskIds[0], undefined, `已创建 115 云下载任务${result.taskIds.length ? `（${result.taskIds.length} 项）` : ''}，等待网盘完成`)
      await rereadMediaAcquisitionStagingTarget(run, candidateId, staging)
      return
    }
    if (platform === 'guangya') {
      const result = await apiGuangyaOfflineCreate(run.target.targetUserId, source.locator, candidate.title, parentId)
      if (result.error) throw new Error(result.error)
      DownDAL.aTrackGuangyaOfflineDownload(run.target.targetUserId, source.locator, candidate.title, parentId, String(result.taskId || result.fileId), result.fileId)
      externalTaskCreated = true
      step = '回读暂存目录'
      await recordMediaAcquisitionExternalTask(run.id, candidateId, result.taskId, result.fileId, '已创建光鸭云盘离线下载任务，等待网盘完成')
      await rereadMediaAcquisitionStagingTarget(run, candidateId, staging)
      return
    }
    const pikpak = await apiPikPakOfflineCreate(run.target.targetUserId, source.locator, candidate.title, parentId)
    if (pikpak.error) throw new Error(pikpak.error)
    DownDAL.aTrackPikPakOfflineDownload(run.target.targetUserId, source.locator, candidate.title, parentId, String(pikpak.taskId || pikpak.fileId), pikpak.fileId)
    externalTaskCreated = true
    step = '回读暂存目录'
    await recordMediaAcquisitionExternalTask(run.id, candidateId, pikpak.taskId || undefined, pikpak.fileId || undefined, '已创建 PikPak 离线下载任务，等待网盘完成')
    await rereadMediaAcquisitionStagingTarget(run, candidateId, staging)
  } catch (error: any) {
    const message = error?.message || '创建云下载任务失败'
    await addMediaAcquisitionEvent(run.id, 'error', 'transfer', `磁力离线在「${step}」失败：${message}`, { tool: 'magnetOffline', candidateId, platform, step, errorName: error?.name || 'Error', externalTaskCreated })
    if (staging && !externalTaskCreated && await recoverMaterializedOfflineCandidate(run, candidateId, staging, message)) return
    if (staging && !externalTaskCreated) await cleanupFailedMediaAcquisitionStagingTarget(run, candidateId, staging)
    await failMediaAcquisitionCandidate(run.id, candidateId, message, canTryNextMediaAcquisitionCandidate(run.target, run.candidates, candidateId, message))
    throw error
  }
}

export async function executeMediaAcquisitionHttpCandidate(run: MediaAcquisitionRunView, candidateId: string): Promise<void> {
  const candidate = run.candidates.find(item => item.id === candidateId)
  if (!candidate) throw new Error('未找到候选资源')
  if (candidate.kind !== 'http') throw new Error('当前候选资源不是 HTTP/HTTPS 链接')
  const platform = normalizeMediaAcquisitionPlatform(run.target.targetPlatform)
  if (!['115', 'guangya', 'pikpak', 'cloud123'].includes(platform)) throw new Error(`${run.target.targetPlatform} 当前不支持 HTTP 外链离线下载`)
  const source = await getMediaAcquisitionCandidateLocator(run.id, candidateId)
  if (!source || !/^https?:\/\//i.test(source.locator)) throw new Error('无效的 HTTP/HTTPS 链接')

  let staging: MediaAcquisitionStagingTarget | undefined
  let externalTaskCreated = false
  let step = '读取入库目录快照'
  try {
    const claimed = await markMediaAcquisitionCandidateTransferring(run.id, candidateId)
    if (!claimed) return
    if (await stopMediaAcquisitionTransferWhenCoverageMet(run)) return
    await addMediaAcquisitionEvent(run.id, 'info', 'transfer', `开始${platformName(platform)} HTTP 外链离线诊断。`, { tool: 'httpOffline', candidateId, platform, targetDriveId: run.target.targetDriveId })
    const baseline = await listMediaAcquisitionTransferBaselineFiles(run.target)
    step = '创建暂存目录'
    staging = await ensureMediaAcquisitionStagingTarget(run.target, run.id, candidateId)
    const parentId = normalizeMediaAcquisitionRootFolder(platform, staging.target.targetParentFileId)
    await recordMediaAcquisitionCandidateBaseline(run.id, candidateId, baseline)
    await addMediaAcquisitionEvent(run.id, 'info', 'transfer', staging.isFinalTarget ? `电影将直接导入最终目录 ${staging.folderName}，准备创建${platformName(platform)}离线下载任务。` : `已创建任务暂存目录 ${staging.folderName}，准备创建${platformName(platform)}离线下载任务。`, { candidateId, platform, title: candidate.title, parentId, folderId: staging.folderId, folderName: staging.folderName, isFinalTarget: staging.isFinalTarget, tool: 'staging' })
    step = '提交离线下载任务'
    await recordMediaAcquisitionTransferIntent(run.id, candidateId, '已记录 HTTP 离线提交意图，正在请求网盘创建任务。')
    if (platform === '115') {
      const result = await apiDrive115OfflineCreate(run.target.targetUserId, source.locator, parentId)
      if (result.error) throw new Error(result.error)
      DownDAL.aTrackDrive115OfflineDownload(run.target.targetUserId, source.locator, parentId, result.taskIds, candidate.title)
      externalTaskCreated = true
      await recordMediaAcquisitionExternalTask(run.id, candidateId, result.taskIds[0], undefined, '已创建 115 HTTP 云下载任务，等待网盘完成')
    } else if (platform === 'guangya') {
      const result = await apiGuangyaOfflineCreate(run.target.targetUserId, source.locator, candidate.title, parentId)
      if (result.error) throw new Error(result.error)
      DownDAL.aTrackGuangyaOfflineDownload(run.target.targetUserId, source.locator, candidate.title, parentId, String(result.taskId || result.fileId), result.fileId)
      externalTaskCreated = true
      await recordMediaAcquisitionExternalTask(run.id, candidateId, result.taskId, result.fileId, '已创建光鸭云盘 HTTP 离线下载任务，等待网盘完成')
    } else if (platform === 'pikpak') {
      const result = await apiPikPakOfflineCreate(run.target.targetUserId, source.locator, candidate.title, parentId)
      if (result.error) throw new Error(result.error)
      DownDAL.aTrackPikPakOfflineDownload(run.target.targetUserId, source.locator, candidate.title, parentId, String(result.taskId || result.fileId), result.fileId)
      externalTaskCreated = true
      await recordMediaAcquisitionExternalTask(run.id, candidateId, result.taskId || undefined, result.fileId || undefined, '已创建 PikPak HTTP 离线下载任务，等待网盘完成')
    } else {
      const result = await apiCloud123OfflineCreate(run.target.targetUserId, source.locator, candidate.title, parentId)
      if (result.error || !result.taskId) throw new Error(result.error || '创建 123 云盘离线下载任务失败')
      DownDAL.aTrackCloud123OfflineDownload(run.target.targetUserId, source.locator, candidate.title, parentId, String(result.taskId))
      externalTaskCreated = true
      await recordMediaAcquisitionExternalTask(run.id, candidateId, String(result.taskId), undefined, '已创建 123 云盘 HTTP 离线下载任务，等待网盘完成')
    }
    step = '回读暂存目录'
    await rereadMediaAcquisitionStagingTarget(run, candidateId, staging)
  } catch (error: any) {
    const message = error?.message || `创建${platformName(platform)}离线下载任务失败`
    await addMediaAcquisitionEvent(run.id, 'error', 'transfer', `HTTP 外链离线在「${step}」失败：${message}`, { tool: 'httpOffline', candidateId, platform, step, errorName: error?.name || 'Error', externalTaskCreated })
    if (staging && !externalTaskCreated && await recoverMaterializedOfflineCandidate(run, candidateId, staging, message)) return
    if (staging && !externalTaskCreated) await cleanupFailedMediaAcquisitionStagingTarget(run, candidateId, staging)
    await failMediaAcquisitionCandidate(run.id, candidateId, message, canTryNextMediaAcquisitionCandidate(run.target, run.candidates, candidateId, message))
    throw error
  }
}

async function recoverMaterializedOfflineCandidate(run: MediaAcquisitionRunView, candidateId: string, staging: MediaAcquisitionStagingTarget, providerError: string): Promise<boolean> {
  if (!await hasMaterializedMediaAcquisitionContent(staging).catch(() => false)) return false
  await addMediaAcquisitionEvent(run.id, 'warning', 'verify', '网盘接口返回异常，但暂存目录已实际落盘媒体文件；以落盘结果为准继续核验。', { tool: 'materializedLandingRecovery', candidateId, folderId: staging.folderId, providerError })
  await recordMediaAcquisitionExternalTask(run.id, candidateId, undefined, undefined, '网盘已落盘媒体文件，正在核对入库目录')
  await rereadMediaAcquisitionStagingTarget(run, candidateId, staging)
  return true
}

function platformName(platform: string): string {
  return ({ '115': '115 云盘', guangya: '光鸭云盘', pikpak: 'PikPak', cloud123: '123 云盘' } as Record<string, string>)[platform] || platform
}
