import type { MediaAcquisitionRunView } from '@shared/types/mediaAcquisition'
import { addMediaAcquisitionEvent, failMediaAcquisitionCandidate, markMediaAcquisitionCandidateTransferring, recordMediaAcquisitionCandidateBaseline, recordMediaAcquisitionExternalTask, recordMediaAcquisitionTransferIntent, submitMediaAcquisitionProviderTransfer } from './client'
import { normalizeMediaAcquisitionPlatform, normalizeMediaAcquisitionRootFolder } from './capabilities'
import { canTryNextMediaAcquisitionCandidate } from './candidatePolicy'
import { listMediaAcquisitionTransferBaselineFiles } from './targetSnapshot'
import { cleanupFailedMediaAcquisitionStagingTarget, ensureMediaAcquisitionStagingTarget, hasMaterializedMediaAcquisitionContent, rereadMediaAcquisitionStagingTarget, stopMediaAcquisitionTransferWhenCoverageMet, type MediaAcquisitionStagingTarget } from './staging'
type ProviderTransferTicket = { workflowId: string; grantId: string }

export async function executeMediaAcquisitionMagnetCandidate(run: MediaAcquisitionRunView, candidateId: string, ticket?: ProviderTransferTicket): Promise<void> {
  const candidate = run.candidates.find(item => item.id === candidateId)
  if (!candidate) throw new Error('未找到候选资源')
  if (candidate.kind !== 'magnet') throw new Error('当前候选资源不是磁力链接')
  const platform = normalizeMediaAcquisitionPlatform(run.target.targetPlatform)
  if (!['115', 'guangya', 'pikpak'].includes(platform)) throw new Error(`${run.target.targetPlatform} 当前不支持磁力离线下载`)

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
    const result = await submitMediaAcquisitionProviderTransfer(run.id, candidateId, parentId, ticket)
    externalTaskCreated = true
    step = '回读暂存目录'
    await recordMediaAcquisitionExternalTask(run.id, candidateId, result.taskId, result.fileId, result.activity)
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

export async function executeMediaAcquisitionHttpCandidate(run: MediaAcquisitionRunView, candidateId: string, ticket?: ProviderTransferTicket): Promise<void> {
  const candidate = run.candidates.find(item => item.id === candidateId)
  if (!candidate) throw new Error('未找到候选资源')
  if (candidate.kind !== 'http') throw new Error('当前候选资源不是 HTTP/HTTPS 链接')
  const platform = normalizeMediaAcquisitionPlatform(run.target.targetPlatform)
  if (!['115', 'guangya', 'pikpak', 'cloud123'].includes(platform)) throw new Error(`${run.target.targetPlatform} 当前不支持 HTTP 外链离线下载`)

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
    const result = await submitMediaAcquisitionProviderTransfer(run.id, candidateId, parentId, ticket)
    externalTaskCreated = true
    await recordMediaAcquisitionExternalTask(run.id, candidateId, result.taskId, result.fileId, result.activity)
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
