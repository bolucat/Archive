import Config from '../../config'
import { apiCloud123OfflineProcess } from '../../cloud123/offline'
import { apiDrive115OfflineProcess } from '../../cloud115/offline'
import { apiGuangyaOfflineProcess } from '../../guangya/offline'
import { apiPikPakOfflineProcess } from '../../pikpak/offline'
import useSettingStore from '../../setting/settingstore'
import type { MediaAcquisitionFileSnapshot, MediaAcquisitionRunView, MediaAcquisitionTarget } from '@shared/types/mediaAcquisition'
import { getMediaAcquisitionCapability } from './capabilities'
import { addMediaAcquisitionEvent } from './client'
import { getAssrtSubtitleCandidateFiles, searchAssrtSubtitleCandidates, type AssrtSubtitleCandidate, type AssrtSubtitleFile } from './assrt'
import { submitExternalUrlOffline, type ExternalUrlOfflineSubmission } from './externalUrlExecutor'
import { buildSidecarSubtitleName, buildSubtitleSearchQuery, isConfirmedNonDomesticMediaOrigin, pickSubtitleReferenceVideo } from './subtitleNaming'

function scoreChineseCandidate(candidate: AssrtSubtitleCandidate): number {
  const text = `${candidate.language} ${candidate.name} ${candidate.videoName}`.toLowerCase()
  return (/(简|繁|中文|chinese|chi|chs|cht|双语)/.test(text) ? 100 : 0) + candidate.score * 10 + Math.min(candidate.downloads, 1000) / 100
}

function selectSubtitleFiles(files: AssrtSubtitleFile[]): AssrtSubtitleFile[] {
  return files.filter(file => /\.(ass|ssa|srt|vtt)$/i.test(file.name))
}

function episodeNumber(name: string): number | undefined {
  const match = name.match(/(?:\bS\d{1,2}[ ._-]*E(?:P)?|\bE(?:P)?|第\s*)(\d{1,3})(?:\s*(?:集|话|話))?/i)
  const value = Number(match?.[1])
  return Number.isInteger(value) && value > 0 ? value : undefined
}

function subtitleTargetName(file: AssrtSubtitleFile, videos: MediaAcquisitionFileSnapshot[], usedNames: Set<string>): string {
  const episode = episodeNumber(file.name)
  const matchingVideo = episode ? videos.find(video => episodeNumber(video.name) === episode) : undefined
  const reference = matchingVideo || (videos.length === 1 ? videos[0] : undefined)
  const initial = reference ? buildSidecarSubtitleName(reference.name, file.name) : file.name
  if (!usedNames.has(initial.toLowerCase())) return initial
  const extension = file.name.match(/\.(ass|ssa|srt|vtt)$/i)?.[0].toLowerCase() || '.srt'
  const base = initial.replace(/\.[^.]+$/, '')
  let ordinal = 2
  while (usedNames.has(`${base}.${ordinal}${extension}`.toLowerCase())) ordinal += 1
  return `${base}.${ordinal}${extension}`
}

function delay(ms: number): Promise<void> {
  return new Promise(resolve => window.setTimeout(resolve, ms))
}

async function isConfirmedNonDomesticMedia(run: MediaAcquisitionRunView): Promise<boolean> {
  if (!run.target.tmdbId) return false
  const type = run.target.mediaType === 'movie' ? 'movie' : 'tv'
  const url = new URL(`${Config.BOXPLAYER_API_URL.replace(/\/+$/, '')}/api/tmdb/proxy/${type}/${run.target.tmdbId}`)
  url.searchParams.set('language', 'zh-CN')
  const response = await fetch(url)
  if (!response.ok) return false
  const payload = await response.json() as any
  const data = payload?.data || payload
  const countries = [
    ...(Array.isArray(data?.origin_country) ? data.origin_country : []),
    ...(Array.isArray(data?.production_countries) ? data.production_countries.map((item: any) => item?.iso_3166_1) : [])
  ].map(value => String(value || '').toUpperCase())
  return isConfirmedNonDomesticMediaOrigin(countries)
}

export async function submitAutoChineseSubtitle(run: MediaAcquisitionRunView, landedFiles: MediaAcquisitionFileSnapshot[] = []): Promise<string> {
  const settings = useSettingStore()
  if (!run.target.fetchSubtitles || !settings.mediaAcquisitionAssrtEnabled) return ''
  const capability = getMediaAcquisitionCapability(run.target.targetPlatform)
  if (!capability?.externalUrlOfflineDownload) {
    const message = `${capability?.label || run.target.targetPlatform} 不支持 HTTP 外链离线下载，已跳过自动字幕`
    await addMediaAcquisitionEvent(run.id, 'info', 'organize', message)
    return message
  }
  try {
    if (!await isConfirmedNonDomesticMedia(run)) {
      const message = '未确认非国产内容，跳过外挂中文字幕补全'
      await addMediaAcquisitionEvent(run.id, 'info', 'organize', message)
      return message
    }
    const query = buildSubtitleSearchQuery(run.target, landedFiles)
    const candidates = (await searchAssrtSubtitleCandidates(query)).sort((a, b) => scoreChineseCandidate(b) - scoreChineseCandidate(a))
    const selected = candidates.find(candidate => scoreChineseCandidate(candidate) >= 100)
    if (!selected) {
      const message = 'ASSRT 未找到合适的中文字幕，已跳过'
      await addMediaAcquisitionEvent(run.id, 'info', 'organize', message)
      return message
    }
    const files = selectSubtitleFiles(await getAssrtSubtitleCandidateFiles(selected.id))
    if (!files.length) {
      const message = 'ASSRT 候选不包含可直接离线的字幕文件，已跳过'
      await addMediaAcquisitionEvent(run.id, 'warning', 'organize', message, { subtitleId: selected.id })
      return message
    }
    const message = await submitSubtitlePackage(run, files, landedFiles, run.target, selected.id, query)
    return message
  } catch (error: any) {
    const message = `自动字幕补全失败，已跳过：${error?.message || '未知错误'}`
    await addMediaAcquisitionEvent(run.id, 'warning', 'organize', message)
    return message
  }
}

export async function viewAutoChineseSubtitleSnapshot(run: MediaAcquisitionRunView, landedFiles: MediaAcquisitionFileSnapshot[] = []): Promise<{ eligible: boolean; reason?: string; query?: string; referenceVideo?: string; candidates: Array<{ id: string; name: string; language: string; score: number }> }> {
  const settings = useSettingStore()
  if (!run.target.fetchSubtitles || !settings.mediaAcquisitionAssrtEnabled) return { eligible: false, reason: '自动中文字幕未启用', candidates: [] }
  const capability = getMediaAcquisitionCapability(run.target.targetPlatform)
  if (!capability?.externalUrlOfflineDownload) return { eligible: false, reason: `${capability?.label || run.target.targetPlatform} 不支持 HTTP 外链离线`, candidates: [] }
  if (!await isConfirmedNonDomesticMedia(run)) return { eligible: false, reason: '未确认非国产内容', candidates: [] }
  const query = buildSubtitleSearchQuery(run.target, landedFiles)
  const candidates = (await searchAssrtSubtitleCandidates(query)).sort((left, right) => scoreChineseCandidate(right) - scoreChineseCandidate(left)).filter(candidate => scoreChineseCandidate(candidate) >= 100)
  return { eligible: true, query, referenceVideo: pickSubtitleReferenceVideo(landedFiles)?.name, candidates: candidates.slice(0, 20).map(candidate => ({ id: String(candidate.id), name: candidate.name, language: candidate.language, score: scoreChineseCandidate(candidate) })) }
}

export async function transferAutoChineseSubtitle(run: MediaAcquisitionRunView, landedFiles: MediaAcquisitionFileSnapshot[], selected: { id: string; name: string; language: string; score: number }, stagingTarget: MediaAcquisitionTarget): Promise<string> {
  const files = selectSubtitleFiles(await getAssrtSubtitleCandidateFiles(Number(selected.id)))
  if (!files.length) throw new Error('所选字幕候选不包含可直接离线的字幕文件')
  return submitSubtitlePackage(run, files, landedFiles, stagingTarget, selected.id)
}

async function submitSubtitlePackage(run: MediaAcquisitionRunView, files: AssrtSubtitleFile[], landedFiles: MediaAcquisitionFileSnapshot[], target: MediaAcquisitionTarget, subtitleId: number | string, query?: string): Promise<string> {
  const videos = landedFiles.filter(file => /\.(mp4|mkv|avi|mov|wmv|flv|webm|m4v|mpg|mpeg|3gp|rmvb|ts|m2ts|mts|vob)$/i.test(file.name) && (file.size === undefined || file.size > 0))
  const usedNames = new Set<string>()
  const outcomes: Array<{ source: string; target: string; verified: boolean; taskId?: string; fileId?: string; error?: string }> = []
  for (const file of files.slice(0, 100)) {
    const targetName = subtitleTargetName(file, videos, usedNames)
    usedNames.add(targetName.toLowerCase())
    try {
      const submission = await submitExternalUrlOffline(target, file.url, targetName)
      outcomes.push({ source: file.name, target: targetName, verified: await waitSubtitleTask(run, submission), taskId: submission.taskId, fileId: submission.fileId })
    } catch (error: any) {
      outcomes.push({ source: file.name, target: targetName, verified: false, error: error?.message || '未知错误' })
    }
  }
  const completed = outcomes.filter(item => item.verified).length
  const submitted = outcomes.filter(item => !item.error).length
  const failed = outcomes.filter(item => item.error).length
  const message = failed ? `字幕包已提交 ${submitted}/${outcomes.length} 条，已确认 ${completed} 条；${failed} 条失败，视频入库不受影响` : `字幕包已提交 ${submitted} 条，已确认 ${completed} 条`
  await addMediaAcquisitionEvent(run.id, failed ? 'warning' : 'info', 'organize', message, { subtitleId, query, stagingFolderId: target.targetParentFileId, outcomes })
  return message
}

async function waitSubtitleTask(run: MediaAcquisitionRunView, submission: ExternalUrlOfflineSubmission): Promise<boolean> {
  if (!submission.taskId && !submission.fileId) return true
  for (let attempt = 0; attempt < 4; attempt += 1) {
    if (attempt > 0) await delay(1500)
    const status = await getSubtitleProcess(run, submission)
    if (status.error) return false
    if (status.failed) return false
    if (status.completed) return true
  }
  return false
}

async function getSubtitleProcess(run: MediaAcquisitionRunView, submission: ExternalUrlOfflineSubmission): Promise<{ completed: boolean; failed: boolean; error?: string }> {
  if (!submission.taskId && submission.fileId) return { completed: true, failed: false }
  if (!submission.taskId) return { completed: false, failed: false }
  if (submission.platform === '115') {
    const value = await apiDrive115OfflineProcess(run.target.targetUserId, submission.taskId)
    return { completed: value.status === 2 || value.process >= 100, failed: value.status < 0, error: value.error }
  }
  if (submission.platform === 'guangya') {
    const value = await apiGuangyaOfflineProcess(run.target.targetUserId, submission.taskId)
    return { completed: value.status === 2 || value.process >= 100, failed: value.status === 3 || value.status === 4, error: value.error }
  }
  if (submission.platform === 'pikpak') {
    const value = await apiPikPakOfflineProcess(run.target.targetUserId, submission.taskId, submission.fileId)
    return { completed: value.status === 2 || value.process >= 100, failed: value.status === 1, error: value.error }
  }
  if (submission.platform === 'cloud123') {
    const value = await apiCloud123OfflineProcess(run.target.targetUserId, submission.taskId)
    return { completed: value.status === 2 || value.process >= 100, failed: value.status < 0, error: value.error }
  }
  return { completed: false, failed: false }
}
