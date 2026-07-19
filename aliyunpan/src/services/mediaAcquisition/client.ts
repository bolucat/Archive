import type { CreateMediaAcquisitionCandidateInput, CreateMediaAcquisitionRunInput, CreateMediaAcquisitionTrackingInput, MediaAcquisitionEvent, MediaAcquisitionFileSnapshot, MediaAcquisitionNotification, MediaAcquisitionPhase, MediaAcquisitionRunView, MediaAcquisitionSeasonTarget, MediaAcquisitionState, MediaAcquisitionTarget, MediaAcquisitionTrackingItem } from '@shared/types/mediaAcquisition'
import { deliverMediaAcquisitionTerminalNotification } from './notificationReporter'
import { requireMediaAcquisitionPro } from '../../utils/usageLimit'

function invoke<T>(channel: string, ...args: unknown[]): Promise<T> {
  const ipc = window.Electron?.ipcRenderer
  if (!ipc) return Promise.reject(new Error('媒体获取功能仅支持桌面客户端'))
  return ipc.invoke(channel, ...args) as Promise<T>
}

function toIpcPayload<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T
}

function terminalRun(result: Promise<MediaAcquisitionRunView | null>): Promise<MediaAcquisitionRunView | null> {
  return result.then(run => {
    deliverMediaAcquisitionTerminalNotification(run)
    return run
  })
}

export function createMediaAcquisitionRun(input: CreateMediaAcquisitionRunInput): Promise<MediaAcquisitionRunView> {
  requireMediaAcquisitionPro()
  return invoke('mediaAcquisition:create', toIpcPayload(input))
}

export function listMediaAcquisitionRuns(limit?: number): Promise<MediaAcquisitionRunView[]> {
  return invoke('mediaAcquisition:list', limit)
}

export function listMediaAcquisitionStates(): Promise<MediaAcquisitionState[]> { return invoke('mediaAcquisition:listStates') }
export function listRunnableMediaAcquisitionRuns(limit?: number): Promise<MediaAcquisitionRunView[]> { return invoke('mediaAcquisition:listRunnable', limit) }
export function claimRunnableMediaAcquisitionRun(workerId: string, runId?: string): Promise<MediaAcquisitionRunView | null> { return invoke('mediaAcquisition:claimRunnable', workerId, runId) }
export function releaseMediaAcquisitionRunClaim(runId: string, workerId: string): Promise<void> { return invoke('mediaAcquisition:releaseClaim', runId, workerId) }
export function renewMediaAcquisitionRunClaim(runId: string, workerId: string): Promise<boolean> { return invoke('mediaAcquisition:renewClaim', runId, workerId) }
export function listMediaAcquisitionTracking(limit?: number): Promise<MediaAcquisitionTrackingItem[]> { return invoke('mediaAcquisition:listTracking', limit) }
export function createMediaAcquisitionTracking(input: CreateMediaAcquisitionTrackingInput): Promise<MediaAcquisitionTrackingItem> {
  requireMediaAcquisitionPro()
  return invoke('mediaAcquisition:createTracking', toIpcPayload(input))
}
export function getMediaAcquisitionTarget(targetId: string): Promise<MediaAcquisitionTarget | null> { return invoke('mediaAcquisition:getTarget', targetId) }
export function endMediaAcquisitionTracking(trackingId: string): Promise<MediaAcquisitionTrackingItem | null> { return invoke('mediaAcquisition:endTracking', trackingId) }
export function upsertMediaAcquisitionTracking(data: Omit<MediaAcquisitionTrackingItem, 'id' | 'tmdbId' | 'title' | 'mediaType' | 'obtainedEpisodes' | 'providerAheadEpisodes' | 'lastCheckedAt'>): Promise<MediaAcquisitionTrackingItem | null> { return invoke('mediaAcquisition:upsertTracking', data) }
export function listMediaAcquisitionNotifications(limit?: number): Promise<MediaAcquisitionNotification[]> { return invoke('mediaAcquisition:listNotifications', limit) }
export function markMediaAcquisitionNotificationsRead(ids?: string[]): Promise<void> { return invoke('mediaAcquisition:markNotificationsRead', ids) }
export function clearMediaAcquisitionNotifications(): Promise<number> { return invoke('mediaAcquisition:clearNotifications') }
export function clearCompletedMediaAcquisitionRuns(): Promise<number> { return invoke('mediaAcquisition:clearCompletedRuns') }
export function getMediaAcquisitionAgentSession(runId: string): Promise<unknown[]> { return invoke('mediaAcquisition:getAgentSession', runId) }
export function saveMediaAcquisitionAgentSession(runId: string, messages: unknown[]): Promise<void> { return invoke('mediaAcquisition:saveAgentSession', runId, toIpcPayload(messages)) }
export function getMediaAcquisitionAgentSandbox(runId: string): Promise<Record<string, unknown>> { return invoke('mediaAcquisition:getAgentSandbox', runId) }
export function saveMediaAcquisitionAgentSandbox(runId: string, state: Record<string, unknown>): Promise<void> { return invoke('mediaAcquisition:saveAgentSandbox', runId, toIpcPayload(state)) }

export function cancelMediaAcquisitionRun(runId: string): Promise<MediaAcquisitionRunView | null> {
  return terminalRun(invoke('mediaAcquisition:cancel', runId))
}

export function beginMediaAcquisitionSearch(runId: string): Promise<MediaAcquisitionRunView | null> {
  return invoke('mediaAcquisition:beginSearch', runId)
}

export function finishMediaAcquisitionSearchWithoutCandidates(runId: string): Promise<MediaAcquisitionRunView | null> {
  return terminalRun(invoke('mediaAcquisition:finishSearchWithoutCandidates', runId))
}

export function retryMediaAcquisitionSearch(runId: string, message: string, delayMs: number, maxRetries?: number): Promise<MediaAcquisitionRunView | null> {
  return invoke('mediaAcquisition:retrySearch', runId, message, delayMs, maxRetries)
}

export function addMediaAcquisitionCandidate(runId: string, input: CreateMediaAcquisitionCandidateInput): Promise<MediaAcquisitionRunView | null> {
  return invoke('mediaAcquisition:addCandidate', runId, input)
}

export function getMediaAcquisitionCandidateLocator(runId: string, candidateId: string): Promise<{ locator: string; password?: string } | null> {
  return invoke('mediaAcquisition:getCandidateLocator', runId, candidateId)
}

export function recordMediaAcquisitionCandidateBaseline(runId: string, candidateId: string, files: MediaAcquisitionFileSnapshot[]): Promise<MediaAcquisitionRunView | null> {
  return invoke('mediaAcquisition:recordCandidateBaseline', runId, candidateId, files)
}

export function getMediaAcquisitionCandidateBaseline(runId: string, candidateId: string): Promise<MediaAcquisitionFileSnapshot[]> {
  return invoke('mediaAcquisition:getCandidateBaseline', runId, candidateId)
}

export function markMediaAcquisitionCandidateTransferring(runId: string, candidateId: string): Promise<MediaAcquisitionRunView | null> {
  return invoke('mediaAcquisition:markCandidateTransferring', runId, candidateId)
}

export function recordMediaAcquisitionTransferIntent(runId: string, candidateId: string, activity: string): Promise<MediaAcquisitionRunView | null> {
  return invoke('mediaAcquisition:recordTransferIntent', runId, candidateId, activity)
}

export function selectMediaAcquisitionCandidate(runId: string, candidateId: string, reason: string): Promise<MediaAcquisitionRunView | null> {
  requireMediaAcquisitionPro()
  return invoke('mediaAcquisition:selectCandidate', runId, candidateId, reason)
}

export function recordMediaAcquisitionExternalTask(runId: string, candidateId: string, taskId?: string, fileId?: string, activity?: string): Promise<MediaAcquisitionRunView | null> {
  return invoke('mediaAcquisition:recordExternalTask', runId, candidateId, taskId, fileId, activity)
}

export function updateMediaAcquisitionExternalTaskProgress(runId: string, candidateId: string, progress: number, activity: string): Promise<MediaAcquisitionRunView | null> {
  return invoke('mediaAcquisition:updateExternalTask', runId, candidateId, progress, activity)
}

export function retryMediaAcquisitionCandidate(runId: string, candidateId: string, message: string, delayMs?: number): Promise<MediaAcquisitionRunView | null> {
  return invoke('mediaAcquisition:retryCandidate', runId, candidateId, message, delayMs)
}

export function markMediaAcquisitionNoCoverage(runId: string, reason: string): Promise<MediaAcquisitionRunView | null> {
  return terminalRun(invoke('mediaAcquisition:noCoverage', runId, reason))
}

export function failMediaAcquisitionRun(runId: string, message: string): Promise<MediaAcquisitionRunView | null> {
  return terminalRun(invoke('mediaAcquisition:failRun', runId, message))
}

export function beginMediaAcquisitionCandidateVerification(runId: string, candidateId: string, activity: string): Promise<MediaAcquisitionRunView | null> {
  return invoke('mediaAcquisition:beginCandidateVerification', runId, candidateId, activity)
}

export function beginMediaAcquisitionOrganizing(runId: string, activity?: string): Promise<MediaAcquisitionRunView | null> {
  return invoke('mediaAcquisition:beginOrganizing', runId, activity)
}

export function completeMediaAcquisitionCandidate(runId: string, candidateId: string, message: string): Promise<MediaAcquisitionRunView | null> {
  return terminalRun(invoke('mediaAcquisition:completeCandidate', runId, candidateId, message))
}

export function completeMediaAcquisitionRun(runId: string, message: string, data?: Record<string, unknown>): Promise<MediaAcquisitionRunView | null> {
  return terminalRun(invoke('mediaAcquisition:completeRun', runId, message, data))
}

export function partialMediaAcquisitionCandidate(runId: string, candidateId: string, message: string): Promise<MediaAcquisitionRunView | null> {
  return terminalRun(invoke('mediaAcquisition:partialCandidate', runId, candidateId, message))
}

export function continueMediaAcquisitionAfterPartial(runId: string, candidateId: string, seasonTargets: MediaAcquisitionSeasonTarget[], message: string): Promise<MediaAcquisitionRunView | null> {
  return invoke('mediaAcquisition:continueAfterPartial', runId, candidateId, toIpcPayload(seasonTargets), message)
}

export function failMediaAcquisitionCandidate(runId: string, candidateId: string, message: string, continueWithAlternatives = false): Promise<MediaAcquisitionRunView | null> {
  return terminalRun(invoke('mediaAcquisition:failCandidate', runId, candidateId, message, continueWithAlternatives))
}

export function addMediaAcquisitionEvent(runId: string, level: MediaAcquisitionEvent['level'], phase: MediaAcquisitionPhase, message: string, data?: Record<string, unknown>): Promise<MediaAcquisitionRunView | null> {
  return invoke('mediaAcquisition:addEvent', runId, level, phase, message, data)
}
