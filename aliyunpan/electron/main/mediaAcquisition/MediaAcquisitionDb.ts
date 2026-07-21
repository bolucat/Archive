import Database from 'better-sqlite3'
import { randomUUID } from 'crypto'
import type { CreateMediaAcquisitionCandidateInput, CreateMediaAcquisitionRunInput, CreateMediaAcquisitionTrackingInput, MediaAcquisitionCandidate, MediaAcquisitionCandidateStatus, MediaAcquisitionDecision, MediaAcquisitionEvent, MediaAcquisitionFileSnapshot, MediaAcquisitionNotification, MediaAcquisitionPhase, MediaAcquisitionRun, MediaAcquisitionRunStatus, MediaAcquisitionRunView, MediaAcquisitionSeasonTarget, MediaAcquisitionState, MediaAcquisitionTarget, MediaAcquisitionTrackingItem } from '@shared/types/mediaAcquisition'
import { isMediaAcquisitionMovieUnreleased, mediaAcquisitionReleaseAt } from '@shared/mediaAcquisitionReleaseGate'

type RunRow = Record<string, unknown>
type TargetRow = Record<string, unknown>
type EventRow = Record<string, unknown>
type CandidateRow = Record<string, unknown>
type DecisionRow = Record<string, unknown>
type TrackingRow = Record<string, unknown>

const TERMINAL_RUN_STATUSES = new Set<MediaAcquisitionRunStatus>(['completed', 'partial', 'no_coverage', 'failed', 'cancelled'])

export class MediaAcquisitionDb {
  private db: Database.Database

  constructor(dbPath: string) {
    this.db = new Database(dbPath)
    this.db.pragma('journal_mode = WAL')
    this.migrate()
  }

  close(): void {
    this.db.close()
  }

  createRun(input: CreateMediaAcquisitionRunInput, now = Date.now()): MediaAcquisitionRunView {
    const mediaKey = mediaKeyOf(input.mediaType, input.tmdbId, input.title, input.year)
    const seasonTargets = normalizeSeasonTargets(input.seasonTargets, input.seasonNumber, input.missingEpisodes)
    const isGapRun = input.kind === 'missing' || input.kind === 'patrol'
    const existing = isGapRun && seasonTargets.length <= 1
      ? this.db.prepare(`SELECT r.* FROM media_acquisition_runs r JOIN media_acquisition_targets t ON t.id = r.target_id WHERE t.media_key = ? AND COALESCE(t.season_number, -1) = ? AND t.target_user_id = ? AND t.target_drive_id = ? AND t.target_parent_file_id = ? AND r.status NOT IN ('failed', 'cancelled', 'no_coverage', 'completed') ORDER BY r.started_at DESC LIMIT 1`).get(mediaKey, input.seasonNumber ?? -1, input.targetUserId, input.targetDriveId, input.targetParentFileId) as RunRow | undefined
      : this.db.prepare(`SELECT r.* FROM media_acquisition_runs r JOIN media_acquisition_targets t ON t.id = r.target_id WHERE t.media_key = ? AND t.target_user_id = ? AND t.target_drive_id = ? AND t.target_parent_file_id = ? AND r.status NOT IN ('failed', 'cancelled', 'no_coverage') ORDER BY r.started_at DESC LIMIT 1`).get(mediaKey, input.targetUserId, input.targetDriveId, input.targetParentFileId) as RunRow | undefined
    const existingStatus = existing ? this.toRun(existing).status : undefined
    if (existing && !(input.force && (existingStatus === 'completed' || existingStatus === 'partial'))) throw new Error(existingStatus === 'completed' ? '该媒体已加入媒体库，不能重复获取' : '该媒体已有进行中的获取任务')
    const targetId = input.existingTargetId || randomUUID()
    const runId = randomUUID()
    const releaseAt = input.mediaType === 'movie' && isMediaAcquisitionMovieUnreleased(input.releaseDate, new Date(now)) ? mediaAcquisitionReleaseAt(input.releaseDate) : undefined
    const initialStatus: MediaAcquisitionRunStatus = releaseAt ? 'reserved' : 'queued'
    const initialActivity = releaseAt ? '电影尚未上映，已预定等待上映后自动获取' : '等待处理'
    const create = this.db.transaction(() => {
      if (input.existingTargetId) {
        const target = this.db.prepare('SELECT id FROM media_acquisition_targets WHERE id = ?').get(input.existingTargetId)
        if (!target) throw new Error('追更目标不存在')
        this.db.prepare('UPDATE media_acquisition_targets SET season_number = ?, missing_episodes_json = ?, season_targets_json = ?, tracking_enabled = 1 WHERE id = ?')
          .run(input.seasonNumber ?? seasonTargets[0]?.seasonNumber ?? null, input.missingEpisodes?.length ? JSON.stringify(input.missingEpisodes) : null, seasonTargets.length ? JSON.stringify(seasonTargets) : null, targetId)
      } else {
        this.db.prepare(`
          INSERT INTO media_acquisition_targets (
            id, media_key, media_library_item_id, tmdb_id, media_type, title, alternative_titles_json, year, season_number, missing_episodes_json, season_targets_json,
            target_user_id, target_drive_id, target_platform, target_parent_file_id,
            preferred_quality, fetch_subtitles, preferred_language, tracking_enabled, created_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `).run(
          targetId, mediaKey, input.mediaLibraryItemId ?? null, input.tmdbId ?? null, input.mediaType, input.title.trim(), stringifyAlternativeTitles(input.alternativeTitles, input.title), input.year ?? null, input.seasonNumber ?? seasonTargets[0]?.seasonNumber ?? null, input.missingEpisodes?.length ? JSON.stringify(input.missingEpisodes) : null, seasonTargets.length ? JSON.stringify(seasonTargets) : null,
          input.targetUserId, input.targetDriveId, input.targetPlatform, input.targetParentFileId,
          input.preferredQuality ?? null, input.fetchSubtitles === false ? 0 : 1, input.preferredLanguage ?? null, input.trackingEnabled ? 1 : 0, now
        )
      }
      this.db.prepare(`
        INSERT INTO media_acquisition_runs (
          id, target_id, kind, status, phase, progress, activity, attempt_count, search_attempt_count, next_attempt_at, started_at, finished_at, error_code, error_message
        ) VALUES (?, ?, ?, ?, 'queued', 0, ?, 0, 0, ?, ?, NULL, NULL, NULL)
      `).run(runId, targetId, input.kind, initialStatus, initialActivity, releaseAt ?? null, now)
      this.appendEvent(runId, 'info', 'queued', releaseAt ? `电影将于 ${input.releaseDate?.slice(0, 10)} 上映，已预定并将在上映后自动开始获取。` : '已创建媒体获取任务，等待任务执行器处理。', undefined, now)
    })
    create()
    return this.getRun(runId)!
  }

  listRuns(limit = 50): MediaAcquisitionRunView[] {
    const rows = this.db.prepare('SELECT * FROM media_acquisition_runs ORDER BY started_at DESC LIMIT ?').all(limit) as RunRow[]
    return rows.map(row => this.toRunView(row))
  }

  listStates(): MediaAcquisitionState[] {
    const rows = this.db.prepare(`SELECT r.*, t.media_key, t.title, t.media_type, t.target_user_id, t.target_drive_id, t.target_parent_file_id FROM media_acquisition_runs r JOIN media_acquisition_targets t ON t.id = r.target_id ORDER BY r.started_at DESC`).all() as RunRow[]
    const seen = new Set<string>()
    return rows.flatMap(row => {
      const mediaKey = String(row.media_key)
      const targetUserId = String(row.target_user_id)
      const targetDriveId = String(row.target_drive_id)
      const targetParentFileId = String(row.target_parent_file_id)
      const stateKey = `${mediaKey}:${targetUserId}:${targetDriveId}:${targetParentFileId}`
      if (seen.has(stateKey)) return []
      seen.add(stateKey)
      const run = this.toRun(row)
      return [{ id: run.id, mediaKey, title: String(row.title), mediaType: row.media_type as MediaAcquisitionState['mediaType'], targetUserId, targetDriveId, targetParentFileId, status: run.status, phase: run.phase, progress: run.progress, activity: run.activity, finishedAt: run.finishedAt }]
    })
  }

  listNotifications(limit = 100): MediaAcquisitionNotification[] {
    return (this.db.prepare('SELECT * FROM media_acquisition_notifications ORDER BY created_at DESC LIMIT ?').all(limit) as EventRow[]).map(row => ({ id: String(row.id), runId: String(row.run_id), mediaKey: String(row.media_key), title: String(row.title), status: row.status as MediaAcquisitionNotification['status'], message: String(row.message), read: Number(row.read_at) > 0, createdAt: Number(row.created_at) }))
  }

  markNotificationsRead(ids?: string[]): void {
    if (ids?.length) this.db.prepare(`UPDATE media_acquisition_notifications SET read_at = ? WHERE id IN (${ids.map(() => '?').join(',')})`).run(Date.now(), ...ids)
    else this.db.prepare('UPDATE media_acquisition_notifications SET read_at = ? WHERE read_at IS NULL').run(Date.now())
  }

  clearNotifications(): number {
    return this.db.prepare('DELETE FROM media_acquisition_notifications').run().changes
  }

  clearCompletedRuns(): number {
    const runIds = (this.db.prepare("SELECT id FROM media_acquisition_runs WHERE status IN ('completed', 'partial', 'no_coverage', 'failed', 'cancelled')").all() as RunRow[]).map(row => String(row.id))
    if (!runIds.length) return 0
    const placeholders = runIds.map(() => '?').join(',')
    this.db.transaction(() => {
      this.db.prepare(`DELETE FROM media_acquisition_notifications WHERE run_id IN (${placeholders})`).run(...runIds)
      this.db.prepare(`DELETE FROM media_acquisition_transfer_attempts WHERE run_id IN (${placeholders})`).run(...runIds)
      this.db.prepare(`DELETE FROM media_acquisition_decisions WHERE run_id IN (${placeholders})`).run(...runIds)
      this.db.prepare(`DELETE FROM media_acquisition_events WHERE run_id IN (${placeholders})`).run(...runIds)
      this.db.prepare(`DELETE FROM media_acquisition_candidates WHERE run_id IN (${placeholders})`).run(...runIds)
      this.db.prepare(`DELETE FROM media_acquisition_runs WHERE id IN (${placeholders})`).run(...runIds)
    })()
    return runIds.length
  }

  getRun(runId: string): MediaAcquisitionRunView | null {
    const row = this.db.prepare('SELECT * FROM media_acquisition_runs WHERE id = ?').get(runId) as RunRow | undefined
    return row ? this.toRunView(row) : null
  }

  listRunnableRuns(limit = 20, now = Date.now()): MediaAcquisitionRunView[] {
    this.activateDueReservations(now)
    const rows = this.db.prepare(`SELECT * FROM media_acquisition_runs WHERE status IN ('queued', 'searching', 'selecting', 'transferring', 'verifying', 'organizing', 'retry_wait') AND (next_attempt_at IS NULL OR next_attempt_at <= ?) ORDER BY started_at ASC LIMIT ?`).all(now, limit) as RunRow[]
    return rows.map(row => this.toRunView(row))
  }

  claimRunnableRun(workerId: string, runId?: string, leaseMs = 120_000, now = Date.now()): MediaAcquisitionRunView | null {
    this.activateDueReservations(now)
    const leaseExpiresAt = now + leaseMs
    const claimedId = this.db.transaction(() => {
      const where = `status IN ('queued', 'searching', 'selecting', 'transferring', 'verifying', 'organizing', 'retry_wait') AND (next_attempt_at IS NULL OR next_attempt_at <= ?) AND (worker_lease_expires_at IS NULL OR worker_lease_expires_at <= ?)`
      const row = runId
        ? this.db.prepare(`SELECT id FROM media_acquisition_runs WHERE id = ? AND ${where}`).get(runId, now, now) as RunRow | undefined
        : this.db.prepare(`SELECT id FROM media_acquisition_runs WHERE ${where} ORDER BY started_at ASC LIMIT 1`).get(now, now) as RunRow | undefined
      if (!row) return ''
      const result = this.db.prepare(`UPDATE media_acquisition_runs SET worker_id = ?, worker_lease_expires_at = ? WHERE id = ? AND ${where}`).run(workerId, leaseExpiresAt, String(row.id), now, now)
      return result.changes ? String(row.id) : ''
    })()
    return claimedId ? this.getRun(claimedId) : null
  }

  releaseRunClaim(runId: string, workerId: string): void {
    this.db.prepare('UPDATE media_acquisition_runs SET worker_id = NULL, worker_lease_expires_at = NULL WHERE id = ? AND worker_id = ?').run(runId, workerId)
  }

  renewRunClaim(runId: string, workerId: string, leaseMs = 120_000, now = Date.now()): boolean {
    return this.db.prepare("UPDATE media_acquisition_runs SET worker_lease_expires_at = ? WHERE id = ? AND worker_id = ? AND status NOT IN ('completed', 'partial', 'no_coverage', 'failed', 'cancelled')").run(now + leaseMs, runId, workerId).changes > 0
  }

  listTracking(limit = 100): MediaAcquisitionTrackingItem[] {
    return (this.db.prepare('SELECT * FROM media_acquisition_tracking ORDER BY next_check_at ASC, created_at DESC LIMIT ?').all(limit) as TrackingRow[]).map(row => this.toTracking(row))
  }

  createTracking(input: CreateMediaAcquisitionTrackingInput, now = Date.now()): MediaAcquisitionTrackingItem {
    const mediaKey = mediaKeyOf(input.mediaType, input.tmdbId, input.title, input.year)
    const existing = this.db.prepare(`
      SELECT tracking.* FROM media_acquisition_tracking tracking
      JOIN media_acquisition_targets target ON target.id = tracking.target_id
      WHERE target.media_key = ? AND target.target_user_id = ? AND target.target_drive_id = ?
        AND target.target_parent_file_id = ? AND tracking.season_number = ? AND tracking.status != 'ended'
      ORDER BY tracking.created_at DESC LIMIT 1
    `).get(mediaKey, input.targetUserId, input.targetDriveId, input.targetParentFileId, input.seasonNumber) as TrackingRow | undefined
    if (existing) return this.toTracking(existing)

    const targetId = randomUUID()
    const trackingId = randomUUID()
    this.db.transaction(() => {
      this.db.prepare(`
        INSERT INTO media_acquisition_targets (
          id, media_key, media_library_item_id, tmdb_id, media_type, title, alternative_titles_json, year, season_number, missing_episodes_json,
          target_user_id, target_drive_id, target_platform, target_parent_file_id,
          preferred_quality, fetch_subtitles, preferred_language, tracking_enabled, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, 1, ?)
      `).run(
        targetId, mediaKey, input.mediaLibraryItemId ?? null, input.tmdbId, input.mediaType, input.title.trim(), stringifyAlternativeTitles(input.alternativeTitles, input.title), input.year ?? null, input.seasonNumber,
        input.targetUserId, input.targetDriveId, input.targetPlatform, input.targetParentFileId,
        input.preferredQuality ?? null, input.fetchSubtitles === false ? 0 : 1, input.preferredLanguage ?? null, now
      )
      this.db.prepare(`
        INSERT INTO media_acquisition_tracking (
          id, target_id, tmdb_id, title, media_type, season_number, total_episodes, latest_aired_episode,
          obtained_episodes, obtained_episodes_json, missing_episodes_json, status, last_checked_at, next_check_at, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, 0, 0, 0, '[]', '[]', 'tracking', NULL, ?, ?)
      `).run(trackingId, targetId, input.tmdbId, input.title.trim(), input.mediaType, input.seasonNumber, now, now)
    })()
    return this.toTracking(this.db.prepare('SELECT * FROM media_acquisition_tracking WHERE id = ?').get(trackingId) as TrackingRow)
  }

  getTarget(targetId: string): MediaAcquisitionTarget | null {
    const row = this.db.prepare('SELECT * FROM media_acquisition_targets WHERE id = ?').get(targetId) as TargetRow | undefined
    return row ? this.toTarget(row) : null
  }

  endTracking(trackingId: string, now = Date.now()): MediaAcquisitionTrackingItem | null {
    this.db.transaction(() => {
      this.db.prepare("UPDATE media_acquisition_tracking SET status = 'ended', last_checked_at = ?, next_check_at = NULL WHERE id = ?").run(now, trackingId)
      this.db.prepare('UPDATE media_acquisition_targets SET tracking_enabled = 0 WHERE id = (SELECT target_id FROM media_acquisition_tracking WHERE id = ?)').run(trackingId)
    })()
    const row = this.db.prepare('SELECT * FROM media_acquisition_tracking WHERE id = ?').get(trackingId) as TrackingRow | undefined
    return row ? this.toTracking(row) : null
  }

  upsertTracking(targetId: string, seasonNumber: number, totalEpisodes: number, latestAiredEpisode: number, obtainedEpisodeNumbers: number[], missingEpisodes: number[], status: MediaAcquisitionTrackingItem['status'], nextCheckAt: number | undefined, now = Date.now()): MediaAcquisitionTrackingItem | null {
    const target = this.db.prepare('SELECT tmdb_id, title, media_type FROM media_acquisition_targets WHERE id = ?').get(targetId) as TargetRow | undefined
    if (!target || !['tv', 'anime'].includes(String(target.media_type))) return null
    const obtained = [...new Set(obtainedEpisodeNumbers)].filter(number => Number.isInteger(number) && number > 0).sort((a, b) => a - b)
    const existing = this.db.prepare('SELECT id FROM media_acquisition_tracking WHERE target_id = ? AND season_number = ?').get(targetId, seasonNumber) as TrackingRow | undefined
    if (existing) {
      this.db.prepare('UPDATE media_acquisition_tracking SET total_episodes = ?, latest_aired_episode = ?, obtained_episodes = ?, obtained_episodes_json = ?, missing_episodes_json = ?, status = ?, last_checked_at = ?, next_check_at = ? WHERE id = ?')
        .run(totalEpisodes, latestAiredEpisode, obtained.length, JSON.stringify(obtained), JSON.stringify(missingEpisodes), status, now, nextCheckAt ?? null, String(existing.id))
      return this.toTracking(this.db.prepare('SELECT * FROM media_acquisition_tracking WHERE id = ?').get(String(existing.id)) as TrackingRow)
    }
    const id = randomUUID()
    this.db.prepare('INSERT INTO media_acquisition_tracking (id, target_id, tmdb_id, title, media_type, season_number, total_episodes, latest_aired_episode, obtained_episodes, obtained_episodes_json, missing_episodes_json, status, last_checked_at, next_check_at, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)')
      .run(id, targetId, target.tmdb_id ?? null, target.title, target.media_type, seasonNumber, totalEpisodes, latestAiredEpisode, obtained.length, JSON.stringify(obtained), JSON.stringify(missingEpisodes), status, now, nextCheckAt ?? null, now)
    return this.toTracking(this.db.prepare('SELECT * FROM media_acquisition_tracking WHERE id = ?').get(id) as TrackingRow)
  }

  cancelRun(runId: string, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || TERMINAL_RUN_STATUSES.has(run.status)) return run
    const endedAfterTransfer = ['transferring', 'verifying', 'organizing'].includes(run.status)
    const eventMessage = endedAfterTransfer
      ? '用户结束了媒体获取任务，停止后续转存核验。'
      : '用户取消了媒体获取任务。'
    const notificationMessage = endedAfterTransfer
      ? '任务已结束，已停止后续转存核验。'
      : '任务已取消，未进入外部转存。'
    const activity = endedAfterTransfer ? '已结束' : '已取消'
    this.db.transaction(() => {
      this.db.prepare("UPDATE media_acquisition_runs SET status = 'cancelled', phase = 'finalize', activity = ?, next_attempt_at = NULL, worker_id = NULL, worker_lease_expires_at = NULL, finished_at = ? WHERE id = ?").run(activity, now, runId)
      this.appendEvent(runId, 'info', 'finalize', eventMessage, { previousStatus: run.status, endedAfterTransfer }, now)
      const target = this.db.prepare('SELECT media_key, title FROM media_acquisition_targets WHERE id = ?').get(run.targetId) as TargetRow
      this.db.prepare('INSERT INTO media_acquisition_notifications (id, run_id, media_key, title, status, message, read_at, created_at) VALUES (?, ?, ?, ?, ?, ?, NULL, ?)').run(randomUUID(), runId, String(target.media_key), String(target.title), 'cancelled', notificationMessage, now)
    })()
    return this.getRun(runId)
  }

  forceCancelRun(runId: string, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || TERMINAL_RUN_STATUSES.has(run.status)) return run
    const endedAfterTransfer = ['transferring', 'verifying', 'organizing'].includes(run.status)
    const eventMessage = endedAfterTransfer ? '用户强制结束了媒体获取任务，已阻止后续转存核验。' : '用户强制取消了媒体获取任务。'
    const notificationMessage = endedAfterTransfer ? '任务已强制结束，已停止后续转存核验。' : '任务已强制取消，未进入外部转存。'
    const activity = endedAfterTransfer ? '已强制结束' : '已强制取消'
    this.db.transaction(() => {
      const result = this.db.prepare("UPDATE media_acquisition_runs SET status = 'cancelled', phase = 'finalize', activity = ?, next_attempt_at = NULL, worker_id = NULL, worker_lease_expires_at = NULL, finished_at = ? WHERE id = ? AND status NOT IN ('completed', 'partial', 'no_coverage', 'failed', 'cancelled')").run(activity, now, runId)
      if (!result.changes) return
      this.appendEvent(runId, 'warning', 'finalize', eventMessage, { forced: true, previousStatus: run.status, endedAfterTransfer }, now)
      const target = this.db.prepare('SELECT media_key, title FROM media_acquisition_targets WHERE id = ?').get(run.targetId) as TargetRow
      this.db.prepare('INSERT INTO media_acquisition_notifications (id, run_id, media_key, title, status, message, read_at, created_at) VALUES (?, ?, ?, ?, ?, ?, NULL, ?)').run(randomUUID(), runId, String(target.media_key), String(target.title), 'cancelled', notificationMessage, now)
    })()
    return this.getRun(runId)
  }

  beginSearch(runId: string, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || run.status !== 'queued') return run
    this.db.transaction(() => {
      this.updateRun(runId, 'searching', 'search', 5, 'Agent 正在搜索可导入资源', undefined, now)
      this.appendEvent(runId, 'info', 'search', 'Agent 开始检索与目标网盘兼容的分享和磁力资源。', undefined, now)
    })()
    return this.getRun(runId)
  }

  finishSearchWithoutCandidates(runId: string, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || run.status !== 'searching') return run
    this.db.transaction(() => {
      this.updateRun(runId, 'no_coverage', 'finalize', 100, '暂未找到可导入资源，可手动添加链接', undefined, now)
      this.appendEvent(runId, 'warning', 'search', 'Agent 未找到与目标网盘兼容的候选资源。', undefined, now)
      const target = this.db.prepare('SELECT media_key, title FROM media_acquisition_targets WHERE id = (SELECT target_id FROM media_acquisition_runs WHERE id = ?)').get(runId) as TargetRow
      this.db.prepare('INSERT INTO media_acquisition_notifications (id, run_id, media_key, title, status, message, read_at, created_at) VALUES (?, ?, ?, ?, ?, ?, NULL, ?)').run(randomUUID(), runId, String(target.media_key), String(target.title), 'no_coverage', '暂未找到可导入资源，可手动添加链接', now)
    })()
    return this.getRun(runId)
  }

  scheduleSearchRetry(runId: string, message: string, delayMs: number, maxRetries: number, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || run.status !== 'searching') return run
    if (run.searchAttemptCount >= maxRetries) return this.finishSearchWithoutCandidates(runId, now)
    const retryNumber = run.searchAttemptCount + 1
    const retryAt = now + delayMs
    this.db.transaction(() => {
      this.db.prepare("UPDATE media_acquisition_runs SET status = 'queued', phase = 'search', progress = 3, activity = ?, search_attempt_count = ?, next_attempt_at = ?, error_message = ? WHERE id = ? AND status = 'searching'")
        .run(`资源暂未可用，等待自动重试（${retryNumber}/${maxRetries}）`, retryNumber, retryAt, message, runId)
      this.appendEvent(runId, 'warning', 'search', `${message}，将在 ${Math.ceil(delayMs / 1000)} 秒后自动重试（${retryNumber}/${maxRetries}）。`, { retryNumber, maxRetries, retryAt, reason: message }, now)
    })()
    return this.getRun(runId)
  }

  failRun(runId: string, message: string, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || TERMINAL_RUN_STATUSES.has(run.status)) return run
    this.db.transaction(() => {
      this.updateRun(runId, 'failed', 'finalize', 100, '获取失败', message, now)
      this.appendEvent(runId, 'error', 'finalize', message, undefined, now)
      const target = this.db.prepare('SELECT media_key, title FROM media_acquisition_targets WHERE id = (SELECT target_id FROM media_acquisition_runs WHERE id = ?)').get(runId) as TargetRow
      this.db.prepare('INSERT INTO media_acquisition_notifications (id, run_id, media_key, title, status, message, read_at, created_at) VALUES (?, ?, ?, ?, ?, ?, NULL, ?)').run(randomUUID(), runId, String(target.media_key), String(target.title), 'failed', message, now)
    })()
    return this.getRun(runId)
  }

  addCandidate(runId: string, input: CreateMediaAcquisitionCandidateInput, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run) return null
    if (!['queued', 'searching', 'selecting', 'no_coverage', 'failed', 'partial'].includes(run.status)) throw new Error('当前任务不能再添加候选资源')
    if (!input.locator.trim()) throw new Error('候选资源链接不能为空')
    const locator = input.locator.trim()
    const locatorKey = candidateLocatorKey(input.kind, input.sourcePlatform, locator)
    const dead = this.db.prepare('SELECT reason FROM media_acquisition_dead_links WHERE locator_key = ? AND expires_at > ?').get(locatorKey, now) as { reason?: string } | undefined
    if (dead) {
      this.appendEvent(runId, 'warning', 'select', `已跳过历史失效候选：${dead.reason || '链接不可用'}`, { kind: input.kind, sourcePlatform: input.sourcePlatform }, now)
      return this.getRun(runId)
    }
    const duplicate = this.db.prepare('SELECT id FROM media_acquisition_candidates WHERE run_id = ? AND locator_key = ? LIMIT 1').get(runId, locatorKey) as CandidateRow | undefined
    if (duplicate) return this.getRun(runId)
    const candidateId = randomUUID()
    this.db.transaction(() => {
      this.db.prepare(`
        INSERT INTO media_acquisition_candidates (
          id, run_id, kind, source_platform, title, detail, locator, locator_key, password, status, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?)
      `).run(candidateId, runId, input.kind, input.sourcePlatform, input.title.trim(), input.detail ?? null, locator, locatorKey, input.password ?? null, now)
      this.db.prepare("UPDATE media_acquisition_runs SET status = 'selecting', phase = 'select', progress = 15, activity = '已找到候选资源，等待确认导入', attempt_count = 0, next_attempt_at = NULL, finished_at = NULL, error_message = NULL WHERE id = ? AND status IN ('queued', 'searching', 'selecting', 'no_coverage', 'failed', 'partial')").run(runId)
      const candidateLabel = input.kind === 'magnet' ? '磁力' : input.kind === 'http' ? 'HTTP 外链' : input.sourcePlatform + ' 分享'
      this.appendEvent(runId, 'info', 'select', `已添加 ${candidateLabel}候选资源。`, { candidateId, kind: input.kind }, now)
    })()
    return this.getRun(runId)
  }

  recordCandidateBaseline(runId: string, candidateId: string, files: MediaAcquisitionFileSnapshot[], now = Date.now()): MediaAcquisitionRunView | null {
    const candidate = this.db.prepare('SELECT id, baseline_json FROM media_acquisition_candidates WHERE id = ? AND run_id = ?').get(candidateId, runId) as CandidateRow | undefined
    if (!candidate) return null
    if (candidate.baseline_json !== null && candidate.baseline_json !== undefined) return this.getRun(runId)
    this.db.transaction(() => {
      this.db.prepare('UPDATE media_acquisition_candidates SET baseline_json = ?, updated_at = ? WHERE id = ?').run(JSON.stringify(files), now, candidateId)
      this.appendEvent(runId, 'info', 'transfer', `已记录入库目录快照（${files.length} 个文件）。`, { candidateId, fileCount: files.length }, now)
    })()
    return this.getRun(runId)
  }

  getCandidateBaseline(runId: string, candidateId: string): MediaAcquisitionFileSnapshot[] {
    const row = this.db.prepare('SELECT baseline_json FROM media_acquisition_candidates WHERE id = ? AND run_id = ?').get(candidateId, runId) as CandidateRow | undefined
    return parseFileSnapshotArray(row?.baseline_json)
  }

  getCandidateLocator(runId: string, candidateId: string): { locator: string; password?: string } | null {
    const row = this.db.prepare('SELECT locator, password FROM media_acquisition_candidates WHERE id = ? AND run_id = ?').get(candidateId, runId) as CandidateRow | undefined
    if (!row) return null
    return { locator: String(row.locator), password: optionalString(row.password) }
  }

  updateCandidateStatus(runId: string, candidateId: string, status: MediaAcquisitionCandidateStatus, now = Date.now()): MediaAcquisitionRunView | null {
    const candidate = this.db.prepare('SELECT id FROM media_acquisition_candidates WHERE id = ? AND run_id = ?').get(candidateId, runId)
    if (!candidate) return null
    this.db.prepare('UPDATE media_acquisition_candidates SET status = ?, updated_at = ? WHERE id = ?').run(status, now, candidateId)
    if (status === 'transferring') this.updateRun(runId, 'transferring', 'transfer', 45, '正在转存候选资源', undefined, now)
    return this.getRun(runId)
  }

  claimCandidateTransfer(runId: string, candidateId: string, now = Date.now()): MediaAcquisitionRunView | null {
    const claimed = this.db.transaction(() => {
      const result = this.db.prepare("UPDATE media_acquisition_candidates SET status = 'transferring', updated_at = ? WHERE id = ? AND run_id = ? AND status = 'selected' AND EXISTS (SELECT 1 FROM media_acquisition_runs WHERE id = ? AND status = 'selecting')").run(now, candidateId, runId, runId)
      if (!result.changes) return false
      this.updateRun(runId, 'transferring', 'transfer', 45, '正在转存候选资源', undefined, now)
      return true
    })()
    return claimed ? this.getRun(runId) : null
  }

  recordTransferIntent(runId: string, candidateId: string, activity: string, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || TERMINAL_RUN_STATUSES.has(run.status) || run.status !== 'transferring' || run.candidates.find(item => item.id === candidateId)?.status !== 'transferring') return run
    this.db.prepare("INSERT INTO media_acquisition_transfer_attempts (id, run_id, candidate_id, status, message, created_at, updated_at) SELECT ?, ?, ?, 'submitting', ?, ?, ? WHERE NOT EXISTS (SELECT 1 FROM media_acquisition_transfer_attempts WHERE run_id = ? AND candidate_id = ? AND status IN ('submitting', 'submitted', 'running'))")
      .run(randomUUID(), runId, candidateId, activity, now, now, runId, candidateId)
    this.appendEvent(runId, 'info', 'transfer', activity, { candidateId, submissionIntent: true }, now)
    return this.getRun(runId)
  }

  selectCandidate(runId: string, candidateId: string, reason: string, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || TERMINAL_RUN_STATUSES.has(run.status) || !['queued', 'searching', 'selecting'].includes(run.status)) return run
    const candidate = run.candidates.find(item => item.id === candidateId)
    if (!candidate || candidate.status !== 'pending') return run
    this.db.transaction(() => {
      const selected = this.db.prepare("UPDATE media_acquisition_candidates SET status = 'selected', selected_at = ?, updated_at = ? WHERE id = ? AND run_id = ? AND status = 'pending' AND NOT EXISTS (SELECT 1 FROM media_acquisition_candidates WHERE run_id = ? AND status IN ('selected', 'transferring')) AND EXISTS (SELECT 1 FROM media_acquisition_runs WHERE id = ? AND status IN ('queued', 'searching', 'selecting'))")
        .run(now, now, candidateId, runId, runId, runId)
      if (!selected.changes) return
      const rejected = this.db.prepare("SELECT id FROM media_acquisition_candidates WHERE run_id = ? AND id != ? AND status = 'pending'").all(runId, candidateId) as CandidateRow[]
      this.db.prepare("UPDATE media_acquisition_candidates SET status = CASE WHEN id = ? THEN 'selected' WHEN status = 'pending' THEN 'rejected' ELSE status END, selected_at = CASE WHEN id = ? THEN ? ELSE selected_at END, updated_at = ? WHERE run_id = ?")
        .run(candidateId, candidateId, now, now, runId)
      this.db.prepare('INSERT INTO media_acquisition_decisions (id, run_id, candidate_id, decision, reason, created_at) VALUES (?, ?, ?, ?, ?, ?)')
        .run(randomUUID(), runId, candidateId, 'select', reason, now)
      for (const item of rejected) {
        this.db.prepare('INSERT INTO media_acquisition_decisions (id, run_id, candidate_id, decision, reason, created_at) VALUES (?, ?, ?, ?, ?, ?)')
          .run(randomUUID(), runId, String(item.id), 'reject', `未入选：${reason}`, now)
      }
      this.updateRun(runId, 'selecting', 'select', 25, 'Agent 已选择最匹配的资源，准备转存', undefined, now)
      this.appendEvent(runId, 'info', 'select', 'Agent 已完成资源筛选。', { candidateId, reason }, now)
    })()
    return this.getRun(runId)
  }

  recordExternalTask(runId: string, candidateId: string, externalTaskId: string | undefined, externalFileId: string | undefined, activity: string, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || TERMINAL_RUN_STATUSES.has(run.status) || run.status !== 'transferring' || run.candidates.find(item => item.id === candidateId)?.status !== 'transferring') return run
    this.db.transaction(() => {
      this.db.prepare("UPDATE media_acquisition_candidates SET status = 'imported', external_task_id = ?, external_file_id = ?, updated_at = ? WHERE id = ?")
        .run(externalTaskId || null, externalFileId || null, now, candidateId)
      const attempt = this.db.prepare("UPDATE media_acquisition_transfer_attempts SET provider_task_id = ?, provider_file_id = ?, status = 'submitted', message = ?, updated_at = ? WHERE id = (SELECT id FROM media_acquisition_transfer_attempts WHERE run_id = ? AND candidate_id = ? AND status = 'submitting' ORDER BY created_at DESC LIMIT 1)")
        .run(externalTaskId || null, externalFileId || null, activity, now, runId, candidateId)
      if (!attempt.changes) this.db.prepare('INSERT INTO media_acquisition_transfer_attempts (id, run_id, candidate_id, provider_task_id, provider_file_id, status, message, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)')
        .run(randomUUID(), runId, candidateId, externalTaskId || null, externalFileId || null, 'submitted', activity, now, now)
      this.updateRun(runId, 'verifying', 'verify', 65, activity, undefined, now)
      this.appendEvent(runId, 'info', 'verify', activity, { candidateId, externalTaskId, externalFileId }, now)
    })()
    return this.getRun(runId)
  }

  updateExternalTaskProgress(runId: string, candidateId: string, progress: number, activity: string, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || TERMINAL_RUN_STATUSES.has(run.status) || !['verifying', 'retry_wait'].includes(run.status) || run.candidates.find(item => item.id === candidateId)?.status !== 'imported') return run
    const normalized = Math.max(65, Math.min(85, 65 + Math.round(Math.max(0, Math.min(100, progress)) * 0.2)))
    this.db.transaction(() => {
      this.db.prepare('UPDATE media_acquisition_candidates SET updated_at = ? WHERE id = ?').run(now, candidateId)
      this.db.prepare("UPDATE media_acquisition_transfer_attempts SET status = 'running', message = ?, updated_at = ? WHERE run_id = ? AND candidate_id = ? AND status IN ('submitted', 'running')")
        .run(activity, now, runId, candidateId)
      this.updateRun(runId, 'verifying', 'verify', normalized, activity, undefined, now)
    })()
    return this.getRun(runId)
  }

  markRetry(runId: string, candidateId: string, message: string, retryAt: number, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || TERMINAL_RUN_STATUSES.has(run.status)) return run
    this.db.transaction(() => {
      this.db.prepare('UPDATE media_acquisition_candidates SET last_error = ?, updated_at = ? WHERE id = ? AND run_id = ?').run(message, now, candidateId, runId)
      this.db.prepare("UPDATE media_acquisition_runs SET status = 'retry_wait', phase = 'verify', activity = ?, next_attempt_at = ?, attempt_count = attempt_count + 1, error_message = ? WHERE id = ? AND status IN ('verifying', 'retry_wait')")
        .run('网盘暂时不可用，等待重试', retryAt, message, runId)
      this.appendEvent(runId, 'warning', 'verify', message, { candidateId, retryAt }, now)
    })()
    return this.getRun(runId)
  }

  markNoCoverage(runId: string, reason: string, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || TERMINAL_RUN_STATUSES.has(run.status)) return run
    this.db.transaction(() => {
      this.db.prepare('INSERT INTO media_acquisition_decisions (id, run_id, candidate_id, decision, reason, created_at) VALUES (?, ?, NULL, ?, ?, ?)')
        .run(randomUUID(), runId, 'no_coverage', reason, now)
      this.updateRun(runId, 'no_coverage', 'finalize', 100, '暂未找到可获取资源', reason, now)
      this.appendEvent(runId, 'warning', 'finalize', reason, undefined, now)
      const target = this.db.prepare('SELECT media_key, title FROM media_acquisition_targets WHERE id = (SELECT target_id FROM media_acquisition_runs WHERE id = ?)').get(runId) as TargetRow
      this.db.prepare('INSERT INTO media_acquisition_notifications (id, run_id, media_key, title, status, message, read_at, created_at) VALUES (?, ?, ?, ?, ?, ?, NULL, ?)').run(randomUUID(), runId, String(target.media_key), String(target.title), 'no_coverage', reason, now)
    })()
    return this.getRun(runId)
  }

  beginVerifyingCandidate(runId: string, candidateId: string, activity: string, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || TERMINAL_RUN_STATUSES.has(run.status) || !['transferring', 'verifying', 'retry_wait'].includes(run.status) || !['transferring', 'imported'].includes(run.candidates.find(item => item.id === candidateId)?.status || '')) return run
    this.db.transaction(() => {
      this.db.prepare("UPDATE media_acquisition_candidates SET status = 'imported', updated_at = ? WHERE id = ?").run(now, candidateId)
      this.updateRun(runId, 'verifying', 'verify', 70, activity, undefined, now)
      this.appendEvent(runId, 'info', 'verify', activity, { candidateId }, now)
    })()
    return this.getRun(runId)
  }

  beginOrganizing(runId: string, activity = '正在整理并扫描入库目录', now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || run.status !== 'verifying') return run
    this.db.transaction(() => {
      this.updateRun(runId, 'organizing', 'organize', 88, activity, undefined, now)
      this.appendEvent(runId, 'info', 'organize', activity, undefined, now)
    })()
    return this.getRun(runId)
  }

  completeCandidate(runId: string, candidateId: string, message: string, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || TERMINAL_RUN_STATUSES.has(run.status) || !['verifying', 'organizing'].includes(run.status) || run.candidates.find(item => item.id === candidateId)?.status !== 'imported') return run
    this.db.transaction(() => {
      this.db.prepare("UPDATE media_acquisition_candidates SET status = 'imported', updated_at = ? WHERE id = ?").run(now, candidateId)
      this.updateRun(runId, 'completed', 'finalize', 100, message, undefined, now)
      this.appendEvent(runId, 'info', 'finalize', message, { candidateId }, now)
      const target = this.db.prepare('SELECT media_key, title FROM media_acquisition_targets WHERE id = (SELECT target_id FROM media_acquisition_runs WHERE id = ?)').get(runId) as TargetRow
      this.db.prepare('INSERT INTO media_acquisition_notifications (id, run_id, media_key, title, status, message, read_at, created_at) VALUES (?, ?, ?, ?, ?, ?, NULL, ?)').run(randomUUID(), runId, String(target.media_key), String(target.title), 'completed', message, now)
    })()
    return this.getRun(runId)
  }

  completeRun(runId: string, message: string, data?: Record<string, unknown>, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || ['completed', 'partial', 'no_coverage', 'failed', 'cancelled'].includes(run.status)) return run
    this.db.transaction(() => {
      this.db.prepare("UPDATE media_acquisition_candidates SET status = 'rejected', updated_at = ? WHERE run_id = ? AND status IN ('selected', 'transferring')").run(now, runId)
      this.updateRun(runId, 'completed', 'finalize', 100, message, undefined, now)
      this.appendEvent(runId, 'info', 'finalize', message, data, now)
      const target = this.db.prepare('SELECT media_key, title FROM media_acquisition_targets WHERE id = (SELECT target_id FROM media_acquisition_runs WHERE id = ?)').get(runId) as TargetRow
      this.db.prepare('INSERT INTO media_acquisition_notifications (id, run_id, media_key, title, status, message, read_at, created_at) VALUES (?, ?, ?, ?, ?, ?, NULL, ?)').run(randomUUID(), runId, String(target.media_key), String(target.title), 'completed', message, now)
    })()
    return this.getRun(runId)
  }

  partialCandidate(runId: string, candidateId: string, message: string, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || TERMINAL_RUN_STATUSES.has(run.status) || !['verifying', 'organizing'].includes(run.status) || run.candidates.find(item => item.id === candidateId)?.status !== 'imported') return run
    this.db.transaction(() => {
      this.db.prepare("UPDATE media_acquisition_candidates SET status = 'imported', updated_at = ? WHERE id = ?").run(now, candidateId)
      this.updateRun(runId, 'partial', 'finalize', 100, message, undefined, now)
      this.appendEvent(runId, 'warning', 'finalize', message, { candidateId }, now)
      const target = this.db.prepare('SELECT media_key, title FROM media_acquisition_targets WHERE id = (SELECT target_id FROM media_acquisition_runs WHERE id = ?)').get(runId) as TargetRow
      this.db.prepare('INSERT INTO media_acquisition_notifications (id, run_id, media_key, title, status, message, read_at, created_at) VALUES (?, ?, ?, ?, ?, ?, NULL, ?)').run(randomUUID(), runId, String(target.media_key), String(target.title), 'partial', message, now)
    })()
    return this.getRun(runId)
  }

  continueAfterPartialCandidate(runId: string, candidateId: string, seasonTargets: MediaAcquisitionSeasonTarget[], message: string, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || TERMINAL_RUN_STATUSES.has(run.status) || !['verifying', 'organizing'].includes(run.status) || run.candidates.find(item => item.id === candidateId)?.status !== 'imported') return run
    const remaining = normalizeSeasonTargets(seasonTargets, run.target.seasonNumber, run.target.missingEpisodes || [])
    const primaryMissing = remaining.find(item => item.seasonNumber === (run.target.seasonNumber || remaining[0]?.seasonNumber))?.missingEpisodes || []
    this.db.transaction(() => {
      this.db.prepare("UPDATE media_acquisition_candidates SET status = 'imported', updated_at = ? WHERE id = ?").run(now, candidateId)
      this.db.prepare("UPDATE media_acquisition_candidates SET status = 'pending', selected_at = NULL, updated_at = ? WHERE run_id = ? AND status = 'rejected'").run(now, runId)
      this.db.prepare('UPDATE media_acquisition_targets SET missing_episodes_json = ?, season_targets_json = ? WHERE id = (SELECT target_id FROM media_acquisition_runs WHERE id = ?)').run(JSON.stringify(primaryMissing), JSON.stringify(remaining), runId)
      this.db.prepare('INSERT INTO media_acquisition_decisions (id, run_id, candidate_id, decision, reason, created_at) VALUES (?, ?, ?, ?, ?, ?)')
        .run(randomUUID(), runId, candidateId, 'retry', message, now)
      this.db.prepare("UPDATE media_acquisition_runs SET status = 'selecting', phase = 'select', progress = 35, activity = '已部分入库，继续补齐缺集', attempt_count = 0, next_attempt_at = NULL, finished_at = NULL, error_message = NULL WHERE id = ? AND status IN ('verifying', 'organizing')").run(runId)
      this.appendEvent(runId, 'warning', 'select', message, { candidateId, seasonTargets: remaining }, now)
    })()
    return this.getRun(runId)
  }

  failCandidate(runId: string, candidateId: string, message: string, continueWithAlternatives = false, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.getRun(runId)
    if (!run || TERMINAL_RUN_STATUSES.has(run.status)) return run
    const candidate = this.db.prepare("SELECT id, kind, source_platform, locator_key, status FROM media_acquisition_candidates WHERE id = ? AND run_id = ? AND status IN ('selected', 'transferring', 'imported')").get(candidateId, runId) as CandidateRow | undefined
    if (!candidate) return run
    const fallback = continueWithAlternatives
      ? this.db.prepare("SELECT id FROM media_acquisition_candidates WHERE run_id = ? AND id != ? AND status IN ('pending', 'rejected') ORDER BY created_at ASC LIMIT 1").get(runId, candidateId) as CandidateRow | undefined
      : undefined
    this.db.transaction(() => {
      this.db.prepare("UPDATE media_acquisition_candidates SET status = 'failed', last_error = ?, updated_at = ? WHERE id = ?").run(message, now, candidateId)
      this.db.prepare("UPDATE media_acquisition_transfer_attempts SET status = 'failed', message = ?, updated_at = ? WHERE run_id = ? AND candidate_id = ? AND status IN ('submitting', 'submitted', 'running')")
        .run(message, now, runId, candidateId)
      this.recordDeadLinkIfTerminal(candidate, message, now)
      if (fallback) {
        this.db.prepare("UPDATE media_acquisition_candidates SET status = 'pending', selected_at = NULL, updated_at = ? WHERE run_id = ? AND status = 'rejected'").run(now, runId)
        this.db.prepare('INSERT INTO media_acquisition_decisions (id, run_id, candidate_id, decision, reason, created_at) VALUES (?, ?, ?, ?, ?, ?)')
          .run(randomUUID(), runId, candidateId, 'retry', `当前候选失败，继续尝试下一个候选：${message}`, now)
        this.db.prepare("UPDATE media_acquisition_runs SET status = 'selecting', phase = 'select', progress = 25, activity = '当前资源失败，正在尝试下一个候选', attempt_count = 0, next_attempt_at = NULL, finished_at = NULL, error_message = NULL WHERE id = ? AND status NOT IN ('completed', 'partial', 'no_coverage', 'failed', 'cancelled')").run(runId)
        this.appendEvent(runId, 'warning', 'select', `当前资源失败，已切换到下一个候选：${message}`, { candidateId, fallbackCandidateId: String(fallback.id) }, now)
        return
      }
      this.updateRun(runId, 'failed', 'finalize', 100, '转存失败', message, now)
      this.appendEvent(runId, 'error', 'finalize', message, { candidateId }, now)
      const target = this.db.prepare('SELECT media_key, title FROM media_acquisition_targets WHERE id = (SELECT target_id FROM media_acquisition_runs WHERE id = ?)').get(runId) as TargetRow
      this.db.prepare('INSERT INTO media_acquisition_notifications (id, run_id, media_key, title, status, message, read_at, created_at) VALUES (?, ?, ?, ?, ?, ?, NULL, ?)').run(randomUUID(), runId, String(target.media_key), String(target.title), 'failed', message, now)
    })()
    return this.getRun(runId)
  }

  addEvent(runId: string, level: MediaAcquisitionEvent['level'], phase: MediaAcquisitionPhase, message: string, data?: Record<string, unknown>, now = Date.now()): MediaAcquisitionRunView | null {
    const run = this.db.prepare('SELECT id FROM media_acquisition_runs WHERE id = ?').get(runId)
    if (!run) return null
    this.appendEvent(runId, level, phase, message, data, now)
    return this.getRun(runId)
  }

  getAgentSession(runId: string): unknown[] {
    const row = this.db.prepare('SELECT agent_session_json FROM media_acquisition_runs WHERE id = ?').get(runId) as RunRow | undefined
    if (!row || typeof row.agent_session_json !== 'string') return []
    try {
      const messages = JSON.parse(row.agent_session_json)
      return Array.isArray(messages) ? messages : []
    } catch {
      return []
    }
  }

  saveAgentSession(runId: string, messages: unknown[]): void {
    if (!this.db.prepare('SELECT id FROM media_acquisition_runs WHERE id = ?').get(runId)) return
    this.db.prepare('UPDATE media_acquisition_runs SET agent_session_json = ? WHERE id = ?').run(JSON.stringify(messages), runId)
  }

  getAgentSandbox(runId: string): Record<string, unknown> {
    const row = this.db.prepare('SELECT agent_sandbox_json FROM media_acquisition_runs WHERE id = ?').get(runId) as RunRow | undefined
    if (!row || typeof row.agent_sandbox_json !== 'string') return {}
    try {
      const state = JSON.parse(row.agent_sandbox_json)
      return state && typeof state === 'object' && !Array.isArray(state) ? state as Record<string, unknown> : {}
    } catch {
      return {}
    }
  }

  saveAgentSandbox(runId: string, state: Record<string, unknown>): void {
    if (!this.db.prepare('SELECT id FROM media_acquisition_runs WHERE id = ?').get(runId)) return
    this.db.prepare('UPDATE media_acquisition_runs SET agent_sandbox_json = ? WHERE id = ?').run(JSON.stringify(state), runId)
  }

  private appendEvent(runId: string, level: MediaAcquisitionEvent['level'], phase: MediaAcquisitionPhase, message: string, data: Record<string, unknown> | undefined, now: number): void {
    this.db.prepare('INSERT INTO media_acquisition_events (id, run_id, level, phase, message, data_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)')
      .run(randomUUID(), runId, level, phase, message, data ? JSON.stringify(data) : null, now)
  }

  private recordDeadLinkIfTerminal(candidate: CandidateRow, message: string, now: number): void {
    const kind = String(candidate.kind)
    const locatorKey = optionalString(candidate.locator_key)
    if (!locatorKey || !isTerminalDeadLink(kind, message)) return
    const sourcePlatform = String(candidate.source_platform || '')
    const ttl = deadLinkTtlMs(kind, sourcePlatform)
    this.db.prepare(`
      INSERT INTO media_acquisition_dead_links (locator_key, kind, reason, expires_at, updated_at)
      VALUES (?, ?, ?, ?, ?)
      ON CONFLICT(locator_key) DO UPDATE SET kind = excluded.kind, reason = excluded.reason, expires_at = excluded.expires_at, updated_at = excluded.updated_at
    `).run(locatorKey, kind, message, now + ttl, now)
  }

  private toRunView(row: RunRow): MediaAcquisitionRunView {
    const targetRow = this.db.prepare('SELECT * FROM media_acquisition_targets WHERE id = ?').get(row.target_id) as TargetRow
    const eventRows = this.db.prepare('SELECT * FROM media_acquisition_events WHERE run_id = ? ORDER BY created_at ASC').all(row.id) as EventRow[]
    const candidateRows = this.db.prepare('SELECT * FROM media_acquisition_candidates WHERE run_id = ? ORDER BY created_at ASC').all(row.id) as CandidateRow[]
    const decisionRows = this.db.prepare('SELECT * FROM media_acquisition_decisions WHERE run_id = ? ORDER BY created_at ASC').all(row.id) as DecisionRow[]
    return { ...this.toRun(row), target: this.toTarget(targetRow), events: eventRows.map(this.toEvent), candidates: candidateRows.map(this.toCandidate), decisions: decisionRows.map(this.toDecision) }
  }

  private updateRun(runId: string, status: MediaAcquisitionRunStatus, phase: MediaAcquisitionPhase, progress: number, activity: string, errorMessage: string | undefined, now: number): void {
    this.db.prepare("UPDATE media_acquisition_runs SET status = ?, phase = ?, progress = ?, activity = ?, error_message = ?, finished_at = ? WHERE id = ? AND status NOT IN ('completed', 'partial', 'no_coverage', 'failed', 'cancelled')")
      .run(status, phase, progress, activity, errorMessage ?? null, ['completed', 'partial', 'no_coverage', 'failed', 'cancelled'].includes(status) ? now : null, runId)
  }

  private activateDueReservations(now: number): void {
    const rows = this.db.prepare("SELECT id FROM media_acquisition_runs WHERE status = 'reserved' AND next_attempt_at IS NOT NULL AND next_attempt_at <= ?").all(now) as RunRow[]
    if (!rows.length) return
    this.db.transaction(() => {
      for (const row of rows) {
        const runId = String(row.id)
        this.db.prepare("UPDATE media_acquisition_runs SET status = 'queued', phase = 'queued', activity = '电影已上映，等待 Agent 开始获取', next_attempt_at = NULL WHERE id = ?").run(runId)
        this.appendEvent(runId, 'info', 'queued', '电影已上映，预定任务已进入获取队列。', undefined, now)
      }
    })()
  }

  private toTarget(row: TargetRow): MediaAcquisitionTarget {
    return {
      id: String(row.id), mediaKey: String(row.media_key), mediaLibraryItemId: optionalString(row.media_library_item_id), tmdbId: optionalNumber(row.tmdb_id), mediaType: row.media_type as MediaAcquisitionTarget['mediaType'], title: String(row.title), alternativeTitles: parseArray(row.alternative_titles_json).filter((value): value is string => typeof value === 'string'), year: optionalNumber(row.year), seasonNumber: optionalNumber(row.season_number), missingEpisodes: parseArray(row.missing_episodes_json).map(Number).filter(Number.isFinite), seasonTargets: normalizeSeasonTargets(parseSeasonTargets(row.season_targets_json), optionalNumber(row.season_number), parseArray(row.missing_episodes_json).map(Number).filter(Number.isFinite)),
      targetUserId: String(row.target_user_id), targetDriveId: String(row.target_drive_id), targetPlatform: String(row.target_platform), targetParentFileId: String(row.target_parent_file_id),
      preferredQuality: optionalString(row.preferred_quality), fetchSubtitles: Number(row.fetch_subtitles) !== 0, preferredLanguage: optionalString(row.preferred_language), trackingEnabled: Number(row.tracking_enabled) === 1, createdAt: Number(row.created_at)
    }
  }

  private toRun(row: RunRow): MediaAcquisitionRun {
    return {
      id: String(row.id), targetId: String(row.target_id), kind: row.kind as MediaAcquisitionRun['kind'], status: row.status as MediaAcquisitionRunStatus, phase: row.phase as MediaAcquisitionPhase,
      progress: Number(row.progress), activity: String(row.activity), attemptCount: Number(row.attempt_count), searchAttemptCount: Number(row.search_attempt_count || 0), nextAttemptAt: optionalNumber(row.next_attempt_at),
      startedAt: Number(row.started_at), finishedAt: optionalNumber(row.finished_at), errorCode: optionalString(row.error_code), errorMessage: optionalString(row.error_message)
    }
  }

  private toEvent(row: EventRow): MediaAcquisitionEvent {
    return { id: String(row.id), runId: String(row.run_id), level: row.level as MediaAcquisitionEvent['level'], phase: row.phase as MediaAcquisitionPhase, message: String(row.message), data: parseJson(row.data_json), createdAt: Number(row.created_at) }
  }

  private toCandidate(row: CandidateRow): MediaAcquisitionCandidate {
    return {
      id: String(row.id), runId: String(row.run_id), kind: row.kind as MediaAcquisitionCandidate['kind'], sourcePlatform: String(row.source_platform),
      title: String(row.title), detail: optionalString(row.detail), status: row.status as MediaAcquisitionCandidateStatus, externalTaskId: optionalString(row.external_task_id), externalFileId: optionalString(row.external_file_id), lastError: optionalString(row.last_error), selectedAt: optionalNumber(row.selected_at), updatedAt: optionalNumber(row.updated_at), createdAt: Number(row.created_at)
    }
  }

  private toDecision(row: DecisionRow): MediaAcquisitionDecision {
    return { id: String(row.id), runId: String(row.run_id), candidateId: optionalString(row.candidate_id), decision: row.decision as MediaAcquisitionDecision['decision'], reason: String(row.reason), createdAt: Number(row.created_at) }
  }

  private toTracking(row: TrackingRow): MediaAcquisitionTrackingItem {
    const missing = parseArray(row.missing_episodes_json).map(Number).filter(Number.isFinite)
    const obtained = parseArray(row.obtained_episodes_json).map(Number).filter(Number.isFinite)
    const latestAiredEpisode = Number(row.latest_aired_episode || 0)
    return { id: String(row.id), targetId: String(row.target_id), tmdbId: optionalNumber(row.tmdb_id), title: String(row.title), mediaType: row.media_type as 'tv' | 'anime', seasonNumber: Number(row.season_number), totalEpisodes: Number(row.total_episodes), latestAiredEpisode, obtainedEpisodes: obtained.length || Number(row.obtained_episodes), obtainedEpisodeNumbers: obtained, providerAheadEpisodes: obtained.filter(episode => episode > latestAiredEpisode), missingEpisodes: missing, status: row.status as MediaAcquisitionTrackingItem['status'], lastCheckedAt: optionalNumber(row.last_checked_at), nextCheckAt: optionalNumber(row.next_check_at) }
  }

  private migrate(): void {
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS media_acquisition_targets (
        id TEXT PRIMARY KEY,
        media_key TEXT,
        media_library_item_id TEXT,
        tmdb_id INTEGER,
        media_type TEXT NOT NULL,
        title TEXT NOT NULL,
        alternative_titles_json TEXT NOT NULL DEFAULT '[]',
        year INTEGER,
        season_number INTEGER,
        missing_episodes_json TEXT,
        season_targets_json TEXT,
        target_user_id TEXT NOT NULL,
        target_drive_id TEXT NOT NULL,
        target_platform TEXT NOT NULL,
        target_parent_file_id TEXT NOT NULL,
        preferred_quality TEXT,
        fetch_subtitles INTEGER NOT NULL DEFAULT 1,
        preferred_language TEXT,
        tracking_enabled INTEGER NOT NULL,
        created_at INTEGER NOT NULL
      );
      CREATE TABLE IF NOT EXISTS media_acquisition_runs (
        id TEXT PRIMARY KEY,
        target_id TEXT NOT NULL REFERENCES media_acquisition_targets(id),
        kind TEXT NOT NULL,
        status TEXT NOT NULL,
        phase TEXT NOT NULL,
        progress INTEGER NOT NULL,
        activity TEXT NOT NULL,
        attempt_count INTEGER NOT NULL,
        search_attempt_count INTEGER NOT NULL DEFAULT 0,
        next_attempt_at INTEGER,
        started_at INTEGER NOT NULL,
        finished_at INTEGER,
        error_code TEXT,
        error_message TEXT,
        agent_session_json TEXT,
        agent_sandbox_json TEXT,
        worker_id TEXT,
        worker_lease_expires_at INTEGER
      );
      CREATE TABLE IF NOT EXISTS media_acquisition_events (
        id TEXT PRIMARY KEY,
        run_id TEXT NOT NULL REFERENCES media_acquisition_runs(id),
        level TEXT NOT NULL,
        phase TEXT NOT NULL,
        message TEXT NOT NULL,
        data_json TEXT,
        created_at INTEGER NOT NULL
      );
      CREATE TABLE IF NOT EXISTS media_acquisition_candidates (
        id TEXT PRIMARY KEY,
        run_id TEXT NOT NULL REFERENCES media_acquisition_runs(id),
        kind TEXT NOT NULL,
        source_platform TEXT NOT NULL,
        title TEXT NOT NULL,
        detail TEXT,
        locator TEXT NOT NULL,
        locator_key TEXT,
        password TEXT,
        status TEXT NOT NULL,
        external_task_id TEXT,
        external_file_id TEXT,
        last_error TEXT,
        baseline_json TEXT,
        selected_at INTEGER,
        updated_at INTEGER,
        created_at INTEGER NOT NULL
      );
      CREATE TABLE IF NOT EXISTS media_acquisition_dead_links (
        locator_key TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        reason TEXT NOT NULL,
        expires_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
      );
      CREATE TABLE IF NOT EXISTS media_acquisition_transfer_attempts (
        id TEXT PRIMARY KEY,
        run_id TEXT NOT NULL REFERENCES media_acquisition_runs(id),
        candidate_id TEXT NOT NULL REFERENCES media_acquisition_candidates(id),
        provider_task_id TEXT,
        provider_file_id TEXT,
        status TEXT NOT NULL,
        message TEXT,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
      );
      CREATE TABLE IF NOT EXISTS media_acquisition_decisions (
        id TEXT PRIMARY KEY,
        run_id TEXT NOT NULL REFERENCES media_acquisition_runs(id),
        candidate_id TEXT,
        decision TEXT NOT NULL,
        reason TEXT NOT NULL,
        created_at INTEGER NOT NULL
      );
      CREATE TABLE IF NOT EXISTS media_acquisition_tracking (
        id TEXT PRIMARY KEY,
        target_id TEXT NOT NULL REFERENCES media_acquisition_targets(id),
        tmdb_id INTEGER,
        title TEXT NOT NULL,
        media_type TEXT NOT NULL,
        season_number INTEGER NOT NULL,
        total_episodes INTEGER NOT NULL,
        latest_aired_episode INTEGER NOT NULL DEFAULT 0,
        obtained_episodes INTEGER NOT NULL,
        obtained_episodes_json TEXT NOT NULL DEFAULT '[]',
        missing_episodes_json TEXT NOT NULL,
        status TEXT NOT NULL,
        last_checked_at INTEGER,
        next_check_at INTEGER,
        created_at INTEGER NOT NULL,
        UNIQUE(target_id, season_number)
      );
      CREATE TABLE IF NOT EXISTS media_acquisition_notifications (
        id TEXT PRIMARY KEY,
        run_id TEXT NOT NULL REFERENCES media_acquisition_runs(id),
        media_key TEXT NOT NULL,
        title TEXT NOT NULL,
        status TEXT NOT NULL,
        message TEXT NOT NULL,
        read_at INTEGER,
        created_at INTEGER NOT NULL
      );
      CREATE INDEX IF NOT EXISTS media_acquisition_runs_started_at ON media_acquisition_runs(started_at DESC);
      CREATE INDEX IF NOT EXISTS media_acquisition_events_run_id ON media_acquisition_events(run_id, created_at);
      CREATE INDEX IF NOT EXISTS media_acquisition_dead_links_expires_at ON media_acquisition_dead_links(expires_at);
      CREATE INDEX IF NOT EXISTS media_acquisition_decisions_run_id ON media_acquisition_decisions(run_id, created_at);
    `)
    const columns = this.db.prepare('PRAGMA table_info(media_acquisition_targets)').all() as Array<{ name: string }>
    if (!columns.some(column => column.name === 'fetch_subtitles')) {
      this.db.exec('ALTER TABLE media_acquisition_targets ADD COLUMN fetch_subtitles INTEGER NOT NULL DEFAULT 1')
    }
    if (!columns.some(column => column.name === 'media_key')) this.db.exec('ALTER TABLE media_acquisition_targets ADD COLUMN media_key TEXT')
    if (!columns.some(column => column.name === 'season_number')) this.db.exec('ALTER TABLE media_acquisition_targets ADD COLUMN season_number INTEGER')
    if (!columns.some(column => column.name === 'missing_episodes_json')) this.db.exec('ALTER TABLE media_acquisition_targets ADD COLUMN missing_episodes_json TEXT')
    if (!columns.some(column => column.name === 'season_targets_json')) this.db.exec('ALTER TABLE media_acquisition_targets ADD COLUMN season_targets_json TEXT')
    const candidateColumns = this.db.prepare('PRAGMA table_info(media_acquisition_candidates)').all() as Array<{ name: string }>
    for (const [name, definition] of Object.entries({ external_task_id: 'TEXT', external_file_id: 'TEXT', last_error: 'TEXT', selected_at: 'INTEGER', updated_at: 'INTEGER', locator_key: 'TEXT', baseline_json: 'TEXT' })) {
      if (!candidateColumns.some(column => column.name === name)) this.db.exec(`ALTER TABLE media_acquisition_candidates ADD COLUMN ${name} ${definition}`)
    }
    this.db.prepare("UPDATE media_acquisition_candidates SET locator_key = kind || ':' || lower(source_platform) || ':' || lower(trim(locator)) WHERE locator_key IS NULL OR locator_key = ''").run()
    this.db.exec(`
      CREATE INDEX IF NOT EXISTS media_acquisition_candidates_run_id ON media_acquisition_candidates(run_id, created_at);
      CREATE INDEX IF NOT EXISTS media_acquisition_candidates_locator_key ON media_acquisition_candidates(locator_key);
    `)
    const targetColumns = this.db.prepare('PRAGMA table_info(media_acquisition_targets)').all() as Array<{ name: string }>
    if (!targetColumns.some(column => column.name === 'alternative_titles_json')) this.db.exec("ALTER TABLE media_acquisition_targets ADD COLUMN alternative_titles_json TEXT NOT NULL DEFAULT '[]'")
    const runColumns = this.db.prepare('PRAGMA table_info(media_acquisition_runs)').all() as Array<{ name: string }>
    if (!runColumns.some(column => column.name === 'search_attempt_count')) this.db.exec('ALTER TABLE media_acquisition_runs ADD COLUMN search_attempt_count INTEGER NOT NULL DEFAULT 0')
    if (!runColumns.some(column => column.name === 'agent_session_json')) this.db.exec('ALTER TABLE media_acquisition_runs ADD COLUMN agent_session_json TEXT')
    if (!runColumns.some(column => column.name === 'agent_sandbox_json')) this.db.exec('ALTER TABLE media_acquisition_runs ADD COLUMN agent_sandbox_json TEXT')
    if (!runColumns.some(column => column.name === 'worker_id')) this.db.exec('ALTER TABLE media_acquisition_runs ADD COLUMN worker_id TEXT')
    if (!runColumns.some(column => column.name === 'worker_lease_expires_at')) this.db.exec('ALTER TABLE media_acquisition_runs ADD COLUMN worker_lease_expires_at INTEGER')
    this.db.exec('CREATE INDEX IF NOT EXISTS media_acquisition_runs_worker_lease ON media_acquisition_runs(worker_lease_expires_at)')
    const trackingColumns = this.db.prepare('PRAGMA table_info(media_acquisition_tracking)').all() as Array<{ name: string }>
    if (!trackingColumns.some(column => column.name === 'latest_aired_episode')) this.db.exec('ALTER TABLE media_acquisition_tracking ADD COLUMN latest_aired_episode INTEGER NOT NULL DEFAULT 0')
    if (!trackingColumns.some(column => column.name === 'obtained_episodes_json')) this.db.exec("ALTER TABLE media_acquisition_tracking ADD COLUMN obtained_episodes_json TEXT NOT NULL DEFAULT '[]'")
    this.db.exec('CREATE INDEX IF NOT EXISTS media_acquisition_tracking_next_check ON media_acquisition_tracking(next_check_at)')
    const legacy = this.db.prepare('SELECT id, media_type, tmdb_id, title, year FROM media_acquisition_targets WHERE media_key IS NULL OR media_key = ?').all('') as TargetRow[]
    const setKey = this.db.prepare('UPDATE media_acquisition_targets SET media_key = ? WHERE id = ?')
    for (const row of legacy) setKey.run(mediaKeyOf(String(row.media_type), optionalNumber(row.tmdb_id), String(row.title), optionalNumber(row.year)), String(row.id))
    this.db.exec('CREATE INDEX IF NOT EXISTS media_acquisition_targets_media_key ON media_acquisition_targets(media_key)')
  }
}

export function mediaKeyOf(mediaType: string, tmdbId: number | undefined, title: string, year?: number): string {
  if (tmdbId) return `${mediaType}:tmdb:${tmdbId}`
  return `${mediaType}:title:${title.trim().toLowerCase().replace(/\s+/g, ' ')}:${year || ''}`
}

function optionalString(value: unknown): string | undefined {
  return typeof value === 'string' && value ? value : undefined
}

function optionalNumber(value: unknown): number | undefined {
  return typeof value === 'number' ? value : undefined
}

function parseJson(value: unknown): Record<string, unknown> | undefined {
  if (typeof value !== 'string' || !value) return undefined
  try {
    const parsed = JSON.parse(value)
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : undefined
  } catch {
    return undefined
  }
}

function parseArray(value: unknown): unknown[] {
  if (typeof value !== 'string' || !value) return []
  try {
    const parsed = JSON.parse(value)
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

function parseSeasonTargets(value: unknown): Array<{ seasonNumber: number; missingEpisodes: number[] }> {
  return parseArray(value).flatMap(item => {
    if (!item || typeof item !== 'object') return []
    const row = item as Record<string, unknown>
    const seasonNumber = Number(row.seasonNumber)
    if (!Number.isInteger(seasonNumber) || seasonNumber < 1) return []
    return [{ seasonNumber, missingEpisodes: parseArray(JSON.stringify(row.missingEpisodes)).map(Number).filter(episode => Number.isInteger(episode) && episode > 0) }]
  })
}

function normalizeSeasonTargets(value: Array<{ seasonNumber: number; missingEpisodes: number[] }> | undefined, seasonNumber?: number, missingEpisodes?: number[]): Array<{ seasonNumber: number; missingEpisodes: number[] }> {
  const source = value?.length ? value : seasonNumber ? [{ seasonNumber, missingEpisodes: missingEpisodes || [] }] : []
  const bySeason = new Map<number, Set<number>>()
  for (const target of source) {
    if (!Number.isInteger(target.seasonNumber) || target.seasonNumber < 1) continue
    const episodes = bySeason.get(target.seasonNumber) || new Set<number>()
    for (const episode of target.missingEpisodes || []) if (Number.isInteger(episode) && episode > 0) episodes.add(episode)
    bySeason.set(target.seasonNumber, episodes)
  }
  return [...bySeason].sort(([left], [right]) => left - right).map(([seasonNumber, episodes]) => ({ seasonNumber, missingEpisodes: [...episodes].sort((left, right) => left - right) }))
}

function stringifyAlternativeTitles(titles: string[] | undefined, primaryTitle: string): string {
  return JSON.stringify([...new Set((titles || []).map(title => title.trim()).filter(title => title && title !== primaryTitle))])
}

function parseFileSnapshotArray(value: unknown): MediaAcquisitionFileSnapshot[] {
  return parseArray(value).flatMap(item => {
    if (!item || typeof item !== 'object') return []
    const row = item as Record<string, unknown>
    const name = optionalString(row.name)
    const path = optionalString(row.path)
    if (!name || !path) return []
    return [{ id: optionalString(row.id) || path, name, path, size: optionalNumber(row.size) }]
  })
}

function candidateLocatorKey(kind: string, sourcePlatform: string, locator: string): string {
  const prefix = `${kind}:${sourcePlatform.trim().toLowerCase()}:`
  if (kind === 'magnet') {
    const hash = locator.match(/btih:([a-z0-9]+)/i)?.[1]
    return `${prefix}${(hash || locator).toLowerCase()}`
  }
  try {
    const url = new URL(locator)
    url.hash = ''
    url.searchParams.sort()
    return `${prefix}${url.hostname.toLowerCase()}${url.pathname.replace(/\/+$/, '')}${url.search}`.toLowerCase()
  } catch {
    return `${prefix}${locator.trim().toLowerCase()}`
  }
}

function deadLinkTtlMs(kind: string, sourcePlatform: string): number {
  // A share can be restored or replaced by its owner, so its cache must be
  // short. HTTP and magnet endpoints are even more volatile. Keep the source
  // in the identity key above to avoid one provider poisoning another.
  if (kind === 'share') return sourcePlatform === 'quark' ? 24 * 60 * 60 * 1000 : 7 * 24 * 60 * 60 * 1000
  return 24 * 60 * 60 * 1000
}

function isTerminalDeadLink(kind: string, message: string): boolean {
  if (/限流|频繁|稍后|重试|超时|timeout|网络|network|quota|rate/i.test(message)) return false
  // An incorrect extraction code is not a dead share. Keep the link reusable so
  // a later manual retry with the right password is not blocked by this cache.
  if (kind === 'share') return /失效|不存在|取消|过期|违规|分享中没有|无法识别|获取.*分享凭证失败/i.test(message)
  return /离线任务失败|创建.*失败|无效的磁力|没有资源|版权|违规|种子|metadata|解析失败|任务失败/i.test(message)
}
