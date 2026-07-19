import AliShare from '../../aliapi/share'
import { apiGuangyaSaveShareFilesBatch, apiGuangyaShareFileList, apiGuangyaShareToken, parseGuangyaShareLink } from '../../guangya/share'
import { apiPikPakSaveShareFilesBatch, apiPikPakShareFileList, apiPikPakShareToken, parsePikPakShareLink } from '../../pikpak/share'
import { apiQuarkSaveShareFilesBatch, apiQuarkShareFileList, apiQuarkShareToken, parseQuarkShareLink } from '../../quark/share'
import { readQuarkCookieStringFromElectron } from '../../quark/auth'
import UserDAL from '../../user/userdal'
import type { CreateMediaAcquisitionCandidateInput, MediaAcquisitionRunView } from '@shared/types/mediaAcquisition'
import { addMediaAcquisitionEvent, failMediaAcquisitionCandidate, getMediaAcquisitionCandidateLocator, markMediaAcquisitionCandidateTransferring, recordMediaAcquisitionCandidateBaseline, recordMediaAcquisitionExternalTask, recordMediaAcquisitionTransferIntent } from './client'
import { canTryNextMediaAcquisitionCandidate } from './candidatePolicy'
import { normalizeMediaAcquisitionPlatform } from './capabilities'
import { listMediaAcquisitionTargetFiles } from './targetSnapshot'
import { cleanupFailedMediaAcquisitionStagingTarget, ensureMediaAcquisitionStagingTarget, hasMaterializedMediaAcquisitionContent, rereadMediaAcquisitionStagingTarget, stopMediaAcquisitionTransferWhenCoverageMet, type MediaAcquisitionStagingTarget } from './staging'

type ParsedShare = { platform: 'aliyun' | 'quark' | 'guangya' | 'pikpak'; shareId: string; password: string }
type PreparedShareImport = { importTo: (target: MediaAcquisitionRunView['target']) => Promise<string> }

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

export async function executeMediaAcquisitionShareCandidate(run: MediaAcquisitionRunView, candidateId: string): Promise<void> {
  const candidate = run.candidates.find(item => item.id === candidateId)
  if (!candidate) throw new Error('未找到候选资源')
  if (candidate.kind !== 'share') throw new Error('当前仅支持执行分享导入候选资源')
  const source = await getMediaAcquisitionCandidateLocator(run.id, candidateId)
  if (!source) throw new Error('候选资源已不存在')
  const parsed = parseMediaAcquisitionShare(source.locator, source.password)
  if (!parsed) throw new Error('无法识别分享链接')
  if (normalizeMediaAcquisitionPlatform(parsed.platform) !== normalizeMediaAcquisitionPlatform(run.target.targetPlatform)) throw new Error(`该分享链接属于${platformName(parsed.platform)}，目标网盘是${platformName(run.target.targetPlatform)}，无法跨网盘转存`)

  let staging: MediaAcquisitionStagingTarget | undefined
  let importSubmitted = false
  let step = '读取分享凭证'
  try {
    const claimed = await markMediaAcquisitionCandidateTransferring(run.id, candidateId)
    if (!claimed) return
    if (await stopMediaAcquisitionTransferWhenCoverageMet(run)) return
    await addMediaAcquisitionEvent(run.id, 'info', 'transfer', `开始${platformName(parsed.platform)}分享导入诊断。`, { tool: 'shareImport', candidateId, sourcePlatform: parsed.platform, targetPlatform: run.target.targetPlatform, targetDriveId: run.target.targetDriveId })
    const prepared = await prepareShareImport(run, parsed)
    step = '读取入库目录快照'
    const importTarget = parsed.platform === 'pikpak' ? { ...run.target, targetParentFileId: 'pikpak_root' } : run.target
    const baseline = await listMediaAcquisitionTargetFiles(importTarget)
    if (parsed.platform !== 'pikpak') {
      step = '创建暂存目录'
      staging = await ensureMediaAcquisitionStagingTarget(run.target, run.id, candidateId)
    }
    await recordMediaAcquisitionCandidateBaseline(run.id, candidateId, baseline)
    if (staging) await addMediaAcquisitionEvent(run.id, 'info', 'transfer', staging.isFinalTarget ? `电影将直接导入最终目录 ${staging.folderName}。` : `已创建任务暂存目录 ${staging.folderName}，转存完成后将核验并整理入库。`, { candidateId, folderId: staging.folderId, folderName: staging.folderName, isFinalTarget: staging.isFinalTarget, tool: 'staging' })
    else await addMediaAcquisitionEvent(run.id, 'info', 'transfer', 'PikPak 分享将先转存到网盘根目录，核验新增文件后再移动到所选入库目录。', { candidateId, tool: 'pikpakRootImport' })
    step = '提交分享转存'
    await recordMediaAcquisitionTransferIntent(run.id, candidateId, '已记录分享转存提交意图，正在请求网盘导入。')
    const result = await prepared.importTo(staging?.target || importTarget)
    if (result !== 'success' && result !== 'async') throw new Error(result || '转存失败')
    importSubmitted = true
    step = '回读暂存目录'
    await recordMediaAcquisitionExternalTask(run.id, candidateId, undefined, undefined, result === 'async' ? '已提交异步分享转存，等待网盘完成' : '分享已转存，正在核对入库目录')
    if (staging) await rereadMediaAcquisitionStagingTarget(run, candidateId, staging)
  } catch (error: any) {
    const message = error?.message || '分享导入失败'
    await addMediaAcquisitionEvent(run.id, 'error', 'transfer', `分享导入在「${step}」失败：${message}`, { tool: 'shareImport', candidateId, step, sourcePlatform: parsed.platform, targetPlatform: run.target.targetPlatform, errorName: error?.name || 'Error', importSubmitted })
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

async function prepareShareImport(run: MediaAcquisitionRunView, parsed: ParsedShare): Promise<PreparedShareImport> {
  if (parsed.platform === 'aliyun') {
    const token = await AliShare.ApiGetShareToken(parsed.shareId, parsed.password)
    if (!token || token.startsWith('，')) throw new Error(token?.replace(/^，/, '') || '获取阿里云盘分享凭证失败')
    const listing = await AliShare.ApiShareFileList(parsed.shareId, token, 'root')
    if (listing.next_marker) throw new Error(`获取阿里云盘分享文件失败：${listing.next_marker}`)
    const files = listing.items || []
    if (!files.length) throw new Error('分享中没有可导入的文件')
    await recordShareSnapshot(run, parsed.platform, files)
    return { importTo: target => AliShare.ApiSaveShareFilesBatch(parsed.shareId, token, target.targetUserId, target.targetDriveId, target.targetParentFileId, files.map(file => file.file_id)) }
  }
  if (parsed.platform === 'quark') {
    const session = await refreshQuarkTargetSession(run.target.targetUserId)
    if (session === 'refreshed') await addMediaAcquisitionEvent(run.id, 'info', 'transfer', '已刷新目标夸克账号登录态，开始读取分享凭证。', { targetUserId: run.target.targetUserId, tool: 'quarkSession' })
    if (session === 'different-account') await addMediaAcquisitionEvent(run.id, 'warning', 'transfer', '当前夸克浏览器登录态属于另一个账号，继续使用目标账号已保存的登录态。', { targetUserId: run.target.targetUserId, tool: 'quarkSession' })
    const token = await apiQuarkShareToken(parsed.shareId, parsed.password, run.target.targetUserId)
    if (!token || token.startsWith('，')) throw new Error(token?.replace(/^，/, '') || '获取夸克分享凭证失败')
    const listing = await apiQuarkShareFileList(parsed.shareId, token, 'root', run.target.targetUserId)
    if (listing.error) throw new Error(listing.error)
    const files = listing.items || []
    if (!files.length) throw new Error('分享中没有可导入的文件')
    await recordShareSnapshot(run, parsed.platform, files)
    return { importTo: target => apiQuarkSaveShareFilesBatch(parsed.shareId, token, target.targetUserId, target.targetParentFileId, files.map(file => file.file_id)) }
  }
  if (parsed.platform === 'guangya') {
    const token = await apiGuangyaShareToken(parsed.shareId, parsed.password)
    if (!token || token.startsWith('，')) throw new Error(token?.replace(/^，/, '') || '获取光鸭云盘分享凭证失败')
    const listing = await apiGuangyaShareFileList(parsed.shareId, token, 'root')
    if (listing.error) throw new Error(listing.error)
    if (!listing.items.length) throw new Error('分享中没有可导入的文件')
    await recordShareSnapshot(run, parsed.platform, listing.items)
    return { importTo: target => apiGuangyaSaveShareFilesBatch(parsed.shareId, token, target.targetUserId, target.targetParentFileId, listing.items.map(file => file.file_id)) }
  }
  const token = await apiPikPakShareToken(parsed.shareId, parsed.password)
  if (!token || token.startsWith('，')) throw new Error(token?.replace(/^，/, '') || '获取 PikPak 分享凭证失败')
  const listing = await apiPikPakShareFileList(parsed.shareId, token, 'root')
  if (listing.error) throw new Error(listing.error)
  if (!listing.items.length) throw new Error('分享中没有可导入的文件')
  await recordShareSnapshot(run, parsed.platform, listing.items)
  return { importTo: target => apiPikPakSaveShareFilesBatch(parsed.shareId, token, target.targetUserId, target.targetParentFileId, listing.items.map(file => file.file_id)) }
}

async function refreshQuarkTargetSession(userId: string): Promise<'refreshed' | 'different-account' | 'unavailable'> {
  const freshCookie = await readQuarkCookieStringFromElectron().catch(() => '')
  if (!freshCookie) return 'unavailable'
  const sessionUid = freshCookie.match(/(?:^|;\s*)__(?:uid|puus)=([^;]+)/)?.[1] || ''
  if (sessionUid && userId !== `quark_${sessionUid}`) return 'different-account'
  const token = UserDAL.GetUserToken(userId) || await UserDAL.GetUserTokenFromDB(userId)
  if (!token) return 'unavailable'
  token.access_token = freshCookie
  UserDAL.SaveUserToken(token)
  return 'refreshed'
}

async function recordShareSnapshot(run: MediaAcquisitionRunView, platform: ParsedShare['platform'], files: Array<{ name?: string; file_id?: string; isDir?: boolean; size?: number | string }>): Promise<void> {
  await addMediaAcquisitionEvent(run.id, 'info', 'transfer', `已读取${platformName(platform)}分享快照：${files.length} 项，准备导入到目标目录。`, {
    platform,
    fileCount: files.length,
    preview: files.slice(0, 12).map(file => ({ name: file.name, isDir: !!file.isDir, size: file.size }))
  })
}

function platformName(platform: string): string {
  return ({ aliyun: '阿里云盘', quark: '夸克网盘', guangya: '光鸭云盘', pikpak: 'PikPak' } as Record<string, string>)[platform] || platform
}
