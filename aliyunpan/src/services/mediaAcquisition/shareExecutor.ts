import { parseGuangyaShareLink } from '../../guangya/share'
import { parsePikPakShareLink } from '../../pikpak/share'
import { parseQuarkShareLink } from '../../quark/share'
import type { CreateMediaAcquisitionCandidateInput, MediaAcquisitionRunView } from '@shared/types/mediaAcquisition'
import { addMediaAcquisitionEvent, failMediaAcquisitionCandidate, markMediaAcquisitionCandidateTransferring, recordMediaAcquisitionCandidateBaseline, recordMediaAcquisitionExternalTask, recordMediaAcquisitionTransferIntent, submitMediaAcquisitionProviderShareImport } from './client'
import { canTryNextMediaAcquisitionCandidate } from './candidatePolicy'
import { normalizeMediaAcquisitionPlatform } from './capabilities'
import { listMediaAcquisitionTargetFiles, listMediaAcquisitionTransferBaselineFiles } from './targetSnapshot'
import { cleanupFailedMediaAcquisitionStagingTarget, ensureMediaAcquisitionStagingTarget, hasMaterializedMediaAcquisitionContent, rereadMediaAcquisitionStagingTarget, stopMediaAcquisitionTransferWhenCoverageMet, type MediaAcquisitionStagingTarget } from './staging'

type ParsedShare = { platform: 'aliyun' | 'quark' | 'guangya' | 'pikpak'; shareId: string; password: string }
type ProviderTransferTicket = { workflowId: string; grantId: string }

export function parseMediaAcquisitionShare(locator: string, password = ''): ParsedShare | null {
  const quark = parseQuarkShareLink(locator)
  if (quark.id) return { platform: 'quark', shareId: quark.id, password: password || quark.pwd }
  const guangya = parseGuangyaShareLink(locator)
  if (guangya.id) return { platform: 'guangya', shareId: guangya.id, password: password || guangya.pwd }
  const pikpak = parsePikPakShareLink(locator)
  if (pikpak.id) return { platform: 'pikpak', shareId: pikpak.id, password: password || pikpak.pwd }
  const aliyun = locator.match(/(?:aliyundrive|alipan)\.com\/s\/([A-Za-z0-9_-]+)/i)?.[1]
  const aliyunPassword = locator.match(/[?&#](?:pwd|password|share_pwd)=([A-Za-z0-9]+)/i)?.[1] || locator.match(/(?:提取码|密码|password|pwd)[:：\s]*([A-Za-z0-9]+)/i)?.[1] || ''
  return aliyun ? { platform: 'aliyun', shareId: aliyun, password: password || aliyunPassword } : null
}

export function createShareCandidateInput(locator: string, password: string, title: string): CreateMediaAcquisitionCandidateInput | null {
  const parsed = parseMediaAcquisitionShare(locator, password)
  return parsed ? { kind: 'share', sourcePlatform: parsed.platform, title, locator, password: parsed.password } : null
}

export function createMediaAcquisitionCandidateInput(locator: string, password: string, title: string): CreateMediaAcquisitionCandidateInput | null {
  const trimmed = locator.trim()
  if (/^magnet:\?xt=urn:btih:/i.test(trimmed)) return { kind: 'magnet', sourcePlatform: 'magnet', title, locator: trimmed }
  if (/^https?:\/\//i.test(trimmed) && !parseMediaAcquisitionShare(trimmed, password)) return { kind: 'http', sourcePlatform: 'http', title, locator: trimmed }
  return createShareCandidateInput(trimmed, password, title)
}

export async function executeMediaAcquisitionShareCandidate(run: MediaAcquisitionRunView, candidateId: string, ticket?: ProviderTransferTicket): Promise<void> {
  const candidate = run.candidates.find(item => item.id === candidateId)
  if (!candidate) throw new Error('未找到候选资源')
  if (candidate.kind !== 'share') throw new Error('当前仅支持执行分享导入候选资源')
  const platform = normalizeMediaAcquisitionPlatform(candidate.sourcePlatform)
  if (!['aliyun', 'quark', 'guangya', 'pikpak'].includes(platform)) throw new Error('无法识别分享链接')
  if (platform !== normalizeMediaAcquisitionPlatform(run.target.targetPlatform)) throw new Error(`该分享链接属于${platformName(platform)}，目标网盘是${platformName(run.target.targetPlatform)}，无法跨网盘转存`)

  let staging: MediaAcquisitionStagingTarget | undefined
  let importSubmitted = false
  let step = '读取分享凭证'
  try {
    const claimed = await markMediaAcquisitionCandidateTransferring(run.id, candidateId)
    if (!claimed) return
    if (await stopMediaAcquisitionTransferWhenCoverageMet(run)) return
    await addMediaAcquisitionEvent(run.id, 'info', 'transfer', `开始${platformName(platform)}分享导入诊断。`, { tool: 'shareImport', candidateId, sourcePlatform: platform, targetPlatform: run.target.targetPlatform, targetDriveId: run.target.targetDriveId })
    step = '读取入库目录快照'
    const importTarget = platform === 'pikpak' ? { ...run.target, targetParentFileId: 'pikpak_root' } : run.target
    const baseline = platform === 'pikpak' ? await listMediaAcquisitionTargetFiles(importTarget) : await listMediaAcquisitionTransferBaselineFiles(run.target)
    if (platform !== 'pikpak') {
      step = '创建暂存目录'
      staging = await ensureMediaAcquisitionStagingTarget(run.target, run.id, candidateId)
    }
    await recordMediaAcquisitionCandidateBaseline(run.id, candidateId, baseline)
    if (staging) await addMediaAcquisitionEvent(run.id, 'info', 'transfer', staging.isFinalTarget ? `电影将直接导入最终目录 ${staging.folderName}。` : `已创建任务暂存目录 ${staging.folderName}，转存完成后将核验并整理入库。`, { candidateId, folderId: staging.folderId, folderName: staging.folderName, isFinalTarget: staging.isFinalTarget, tool: 'staging' })
    else await addMediaAcquisitionEvent(run.id, 'info', 'transfer', 'PikPak 分享将先转存到网盘根目录，核验新增文件后再移动到所选入库目录。', { candidateId, tool: 'pikpakRootImport' })
    step = '提交分享转存'
    await recordMediaAcquisitionTransferIntent(run.id, candidateId, '已记录分享转存提交意图，正在请求网盘导入。')
    const result = await submitMediaAcquisitionProviderShareImport(run.id, candidateId, (staging?.target || importTarget).targetParentFileId, ticket)
    importSubmitted = true
    step = '回读暂存目录'
    await addMediaAcquisitionEvent(run.id, 'info', 'transfer', `主进程已读取分享快照：${result.fileCount} 项，准备核验入库目录。`, { tool: 'shareImport', candidateId, platform, fileCount: result.fileCount })
    await recordMediaAcquisitionExternalTask(run.id, candidateId, undefined, undefined, result.activity)
    if (staging) await rereadMediaAcquisitionStagingTarget(run, candidateId, staging)
  } catch (error: any) {
    const message = error?.message || '分享导入失败'
    await addMediaAcquisitionEvent(run.id, 'error', 'transfer', `分享导入在「${step}」失败：${message}`, { tool: 'shareImport', candidateId, step, sourcePlatform: platform, targetPlatform: run.target.targetPlatform, errorName: error?.name || 'Error', importSubmitted })
    if (staging && !importSubmitted && await hasMaterializedMediaAcquisitionContent(staging).catch(() => false)) {
      await addMediaAcquisitionEvent(run.id, 'warning', 'verify', '网盘接口返回异常，但暂存目录已实际落盘媒体文件；以落盘结果为准继续核验。', { tool: 'materializedLandingRecovery', candidateId, folderId: staging.folderId, providerError: message })
      await recordMediaAcquisitionExternalTask(run.id, candidateId, undefined, undefined, '网盘已落盘媒体文件，正在核对入库目录')
      await rereadMediaAcquisitionStagingTarget(run, candidateId, staging)
      return
    }
    if (staging && !importSubmitted) await cleanupFailedMediaAcquisitionStagingTarget(run, candidateId, staging)
    await failMediaAcquisitionCandidate(run.id, candidateId, message, canTryNextMediaAcquisitionCandidate(run.target, run.candidates, candidateId, message))
    throw error
  }
}

function platformName(platform: string): string {
  return ({ aliyun: '阿里云盘', quark: '夸克网盘', guangya: '光鸭云盘', pikpak: 'PikPak' } as Record<string, string>)[platform] || platform
}
