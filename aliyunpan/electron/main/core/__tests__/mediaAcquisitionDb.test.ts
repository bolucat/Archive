import { afterEach, describe, expect, it } from 'vitest'
import { mkdtempSync, rmSync } from 'fs'
import { join } from 'path'
import { tmpdir } from 'os'
import { MediaAcquisitionDb } from '../../mediaAcquisition/MediaAcquisitionDb.ts'

const tempDirs: string[] = []

function createDb() {
  const dir = mkdtempSync(join(tmpdir(), 'boxplayer-media-acquisition-'))
  tempDirs.push(dir)
  return new MediaAcquisitionDb(join(dir, 'media-acquisition.db'))
}

afterEach(() => {
  while (tempDirs.length) rmSync(tempDirs.pop()!, { recursive: true, force: true })
})

describe('MediaAcquisitionDb', () => {
  it('persists the selected target drive and folder with the queued run', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'season', mediaLibraryItemId: 'library-item-1', tmdbId: 100, mediaType: 'tv', title: '示例剧集', year: 2026,
      targetUserId: 'user-115', targetDriveId: 'drive-115', targetPlatform: '115', targetParentFileId: 'folder-tv',
      preferredQuality: '1080p', preferredLanguage: 'zh-CN', trackingEnabled: true
    }, 1000)

    expect(run.status).toBe('queued')
    expect(run.target).toMatchObject({ targetUserId: 'user-115', targetDriveId: 'drive-115', targetParentFileId: 'folder-tv', targetPlatform: '115' })
    expect(run.events).toEqual([expect.objectContaining({ phase: 'queued', message: expect.stringContaining('已创建') })])
    expect(db.getRun(run.id)).toEqual(run)
    db.close()
  })

  it('cancels runs before transfer and ends runs that are already being verified', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'movie', mediaType: 'movie', title: '示例电影', targetUserId: 'user-1', targetDriveId: 'drive-1', targetPlatform: 'aliyun', targetParentFileId: 'movies', trackingEnabled: false
    }, 1000)

    const cancelled = db.cancelRun(run.id, 2000)!
    expect(cancelled.status).toBe('cancelled')
    expect(cancelled.finishedAt).toBe(2000)
    expect(cancelled.events.at(-1)).toMatchObject({ phase: 'finalize', message: expect.stringContaining('取消') })
    expect(db.listNotifications()).toEqual([expect.objectContaining({ title: '示例电影', status: 'cancelled', message: '任务已取消，未进入外部转存。', read: false })])
    expect(db.cancelRun(run.id)).toEqual(cancelled)

    const selecting = db.createRun({
      kind: 'movie', mediaType: 'movie', title: '可取消电影', targetUserId: 'user-1', targetDriveId: 'drive-1', targetPlatform: 'aliyun', targetParentFileId: 'movies', trackingEnabled: false
    }, 3000)
    db.addCandidate(selecting.id, { kind: 'share', sourcePlatform: 'aliyun', title: '可取消电影', locator: 'https://www.alipan.com/s/cancelable' }, 3100)
    expect(db.cancelRun(selecting.id, 3200)).toMatchObject({ status: 'cancelled', finishedAt: 3200 })

    const transferring = db.createRun({
      kind: 'movie', mediaType: 'movie', title: '转存中电影', targetUserId: 'user-1', targetDriveId: 'drive-1', targetPlatform: 'aliyun', targetParentFileId: 'movies', trackingEnabled: false
    }, 4000)
    const candidate = db.addCandidate(transferring.id, { kind: 'share', sourcePlatform: 'aliyun', title: '转存中电影', locator: 'https://www.alipan.com/s/transferring' }, 4100)!.candidates[0]
    db.updateCandidateStatus(transferring.id, candidate.id, 'transferring', 4200)
    db.recordExternalTask(transferring.id, candidate.id, 'guangya-task-1', undefined, '等待光鸭云盘完成', 4250)
    const ended = db.cancelRun(transferring.id, 4300)!
    expect(ended).toMatchObject({ status: 'cancelled', finishedAt: 4300, activity: '已结束' })
    expect(ended.events.at(-1)).toMatchObject({ phase: 'finalize', message: expect.stringContaining('结束') })
    expect(db.listNotifications()).toContainEqual(expect.objectContaining({ title: '转存中电影', status: 'cancelled', message: '任务已结束，已停止后续转存核验。' }))
    expect(db.listRunnableRuns(10, 5000).some(item => item.id === transferring.id)).toBe(false)
    db.updateCandidateStatus(transferring.id, candidate.id, 'transferring', 4350)
    expect(db.getRun(transferring.id)?.status).toBe('cancelled')
    db.close()
  })

  it('retries empty resource searches before recording no coverage', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'movie', mediaType: 'movie', title: '暂无资源电影', targetUserId: 'user-1', targetDriveId: 'drive-1', targetPlatform: '115', targetParentFileId: 'movies', trackingEnabled: false
    }, 1000)
    db.beginSearch(run.id, 1100)

    const waiting = db.scheduleSearchRetry(run.id, '搜索成功但未返回可导入资源', 10_000, 3, 1200)!
    expect(waiting).toMatchObject({ status: 'queued', phase: 'search', searchAttemptCount: 1, nextAttemptAt: 11_200 })
    expect(db.listRunnableRuns(10, 11_199)).toEqual([])
    expect(db.listRunnableRuns(10, 11_200)).toEqual([expect.objectContaining({ id: run.id, status: 'queued' })])

    db.beginSearch(run.id, 11_200)
    db.scheduleSearchRetry(run.id, '搜索成功但未返回可导入资源', 10_000, 3, 11_300)
    db.beginSearch(run.id, 21_300)
    db.scheduleSearchRetry(run.id, '搜索成功但未返回可导入资源', 10_000, 3, 21_400)
    db.beginSearch(run.id, 31_400)
    const exhausted = db.scheduleSearchRetry(run.id, '搜索成功但未返回可导入资源', 10_000, 3, 31_500)!
    expect(exhausted).toMatchObject({ status: 'no_coverage', searchAttemptCount: 3 })
    expect(exhausted.events.at(-1)).toMatchObject({ message: expect.stringContaining('未找到') })
    db.close()
  })

  it('keeps a share locator private while exposing its candidate summary to the task view', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'movie', mediaType: 'movie', title: '示例电影', targetUserId: 'user-1', targetDriveId: 'drive-1', targetPlatform: 'aliyun', targetParentFileId: 'movies', trackingEnabled: false
    }, 1000)

    const updated = db.addCandidate(run.id, {
      kind: 'share', sourcePlatform: 'aliyun', title: '示例电影 1080P', locator: 'https://www.alipan.com/s/private-share-id', password: 'a1b2'
    }, 2000)!

    expect(updated.status).toBe('selecting')
    expect(updated.candidates).toEqual([expect.objectContaining({ kind: 'share', sourcePlatform: 'aliyun', title: '示例电影 1080P', status: 'pending' })])
    expect(JSON.stringify(updated)).not.toContain('private-share-id')
    expect(db.getCandidateLocator(run.id, updated.candidates[0].id)).toEqual({ locator: 'https://www.alipan.com/s/private-share-id', password: 'a1b2' })
    db.close()
  })

  it('deduplicates an acquired media and persists its completion notification', () => {
    const db = createDb()
    const input = {
      kind: 'movie' as const, tmdbId: 42, mediaType: 'movie' as const, title: '示例电影', year: 2026,
      targetUserId: 'user-1', targetDriveId: 'drive-1', targetPlatform: 'aliyun', targetParentFileId: 'movies', trackingEnabled: false
    }
    const run = db.createRun(input, 1000)
    const candidate = db.addCandidate(run.id, { kind: 'share', sourcePlatform: 'aliyun', title: '示例电影', locator: 'https://www.alipan.com/s/share' }, 1100)!.candidates[0]
    db.selectCandidate(run.id, candidate.id, '匹配媒体信息', 1150)
    db.claimCandidateTransfer(run.id, candidate.id, 1175)
    expect(db.beginVerifyingCandidate(run.id, candidate.id, '正在核对入库目录', 1200)?.status).toBe('verifying')
    expect(db.beginOrganizing(run.id, undefined, 1300)?.status).toBe('organizing')
    expect(db.completeCandidate(run.id, candidate.id, '入库完成，已加入媒体库', 1400)?.status).toBe('completed')
    expect(db.listStates()).toEqual([expect.objectContaining({ mediaKey: 'movie:tmdb:42', status: 'completed', progress: 100 })])
    expect(db.listNotifications()).toEqual([expect.objectContaining({ title: '示例电影', status: 'completed', message: '入库完成，已加入媒体库', read: false })])
    expect(() => db.createRun(input, 1500)).toThrow('不能重复获取')
    const repeated = db.createRun({ ...input, force: true }, 1600)
    expect(repeated).toMatchObject({ status: 'queued' })
    db.forceCancelRun(repeated.id, 1700)
    expect(db.createRun({ ...input, kind: 'patrol' }, 1800)).toMatchObject({ kind: 'patrol', status: 'queued' })
    db.close()
  })

  it('clears completed task history without touching active tasks or tracking metadata', () => {
    const db = createDb()
    const finished = db.createRun({ kind: 'movie', mediaType: 'movie', title: '已完成电影', targetUserId: 'user-1', targetDriveId: 'drive-1', targetPlatform: 'aliyun', targetParentFileId: 'movies', trackingEnabled: false }, 1000)
    const candidate = db.addCandidate(finished.id, { kind: 'share', sourcePlatform: 'aliyun', title: '已完成电影', locator: 'https://www.alipan.com/s/finished' }, 1100)!.candidates[0]
    db.selectCandidate(finished.id, candidate.id, '匹配媒体信息', 1125)
    db.claimCandidateTransfer(finished.id, candidate.id, 1150)
    db.beginVerifyingCandidate(finished.id, candidate.id, '核对入库目录', 1175)
    db.completeCandidate(finished.id, candidate.id, '已加入媒体库', 1200)
    const active = db.createRun({ kind: 'movie', mediaType: 'movie', title: '进行中电影', targetUserId: 'user-1', targetDriveId: 'drive-1', targetPlatform: 'aliyun', targetParentFileId: 'movies', trackingEnabled: false }, 1300)

    expect(db.clearCompletedRuns()).toBe(1)
    expect(db.listRuns()).toEqual([expect.objectContaining({ id: active.id, status: 'queued' })])
    expect(db.listNotifications()).toEqual([])
    expect(db.getTarget(finished.targetId)).toMatchObject({ id: finished.targetId })
    db.close()
  })

  it('completes a run without a candidate when requested episodes already exist', () => {
    const db = createDb()
    const input = {
      kind: 'patrol' as const, tmdbId: 100, mediaType: 'tv' as const, title: '示例剧集', year: 2026, seasonNumber: 1, missingEpisodes: [1, 2],
      targetUserId: 'user-115', targetDriveId: 'drive-115', targetPlatform: '115', targetParentFileId: 'season-1', trackingEnabled: true
    }
    const run = db.createRun(input, 1000)
    const completed = db.completeRun(run.id, '目标缺集已在目录中存在，跳过搜索和转存：E1、E2', { skipReason: 'ALREADY_EXISTS' }, 1200)!

    expect(completed).toMatchObject({ status: 'completed', phase: 'finalize', progress: 100, finishedAt: 1200 })
    expect(completed.events.at(-1)).toMatchObject({ phase: 'finalize', message: expect.stringContaining('跳过搜索和转存') })
    expect(db.listNotifications()).toEqual([expect.objectContaining({ title: '示例剧集', status: 'completed', message: expect.stringContaining('跳过搜索和转存'), read: false })])
    expect(db.createRun(input, 1300)).toMatchObject({ kind: 'patrol', status: 'queued' })
    db.close()
  })

  it('clears an in-flight candidate when the final transfer coverage gate completes the run', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'patrol', mediaType: 'tv', title: 'Coverage Gate', seasonNumber: 1, missingEpisodes: [1],
      targetUserId: 'user-115', targetDriveId: 'drive-115', targetPlatform: '115', targetParentFileId: 'season-1', trackingEnabled: true
    }, 1000)
    const candidate = db.addCandidate(run.id, { kind: 'magnet', sourcePlatform: 'magnet', title: 'Coverage.Gate.S01E01', locator: 'magnet:?xt=urn:btih:coverage' }, 1100)!.candidates[0]
    db.selectCandidate(run.id, candidate.id, '等待执行', 1200)
    db.claimCandidateTransfer(run.id, candidate.id, 1300)

    const completed = db.completeRun(run.id, '目标缺集已存在，跳过网盘转存', { tool: 'transferCoverageGate' }, 1400)!
    expect(completed).toMatchObject({ status: 'completed', candidates: [expect.objectContaining({ id: candidate.id, status: 'rejected' })] })
    db.close()
  })

  it('persists the agent decision and external task so a restarted workflow can resume polling', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'movie', mediaType: 'movie', title: 'The Matrix', year: 1999,
      targetUserId: 'user-115', targetDriveId: 'drive-115', targetPlatform: '115', targetParentFileId: 'movies', trackingEnabled: false
    }, 1000)
    const candidate = db.addCandidate(run.id, { kind: 'magnet', sourcePlatform: 'magnet', title: 'The.Matrix.1999.1080p', locator: 'magnet:?xt=urn:btih:abc' }, 1100)!.candidates[0]

    expect(db.selectCandidate(run.id, candidate.id, '匹配年份和 1080p 偏好', 1200)?.candidates[0]).toMatchObject({ status: 'selected', selectedAt: 1200 })
    db.claimCandidateTransfer(run.id, candidate.id, 1250)
    const waiting = db.recordExternalTask(run.id, candidate.id, '115-task-hash', undefined, '等待 115 云下载完成', 1300)!
    expect(waiting).toMatchObject({ status: 'verifying', candidates: [expect.objectContaining({ externalTaskId: '115-task-hash', status: 'imported' })] })
    expect(waiting.decisions).toEqual([expect.objectContaining({ decision: 'select', candidateId: candidate.id })])
    expect(db.listRunnableRuns(10, 1400)).toEqual([expect.objectContaining({ id: run.id, status: 'verifying' })])
    db.close()
  })

  it('persists candidate baselines and recovers in-flight states after restart', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'movie', mediaType: 'movie', title: 'Recoverable Movie',
      targetUserId: 'user-115', targetDriveId: 'drive-115', targetPlatform: '115', targetParentFileId: 'movies', trackingEnabled: false
    }, 1000)
    const candidate = db.addCandidate(run.id, { kind: 'magnet', sourcePlatform: 'magnet', title: 'Recoverable.Movie.1080p', locator: 'magnet:?xt=urn:btih:recoverable' }, 1100)!.candidates[0]
    db.recordCandidateBaseline(run.id, candidate.id, [{ id: 'old', name: 'old.mkv', path: '/old.mkv', size: 1024 }], 1200)
    expect(db.getCandidateBaseline(run.id, candidate.id)).toEqual([{ id: 'old', name: 'old.mkv', path: '/old.mkv', size: 1024 }])
    expect(db.selectCandidate(run.id, candidate.id, '恢复测试', 1300)?.status).toBe('selecting')
    expect(db.updateCandidateStatus(run.id, candidate.id, 'transferring', 1400)?.status).toBe('transferring')
    expect(db.listRunnableRuns(10, 1500)).toEqual([expect.objectContaining({ id: run.id, status: 'transferring' })])
    expect(db.beginVerifyingCandidate(run.id, candidate.id, '恢复后核验', 1600)?.status).toBe('verifying')
    expect(db.beginOrganizing(run.id, undefined, 1700)?.status).toBe('organizing')
    expect(db.listRunnableRuns(10, 1800)).toEqual([expect.objectContaining({ id: run.id, status: 'organizing' })])
    db.close()
  })

  it('claims a candidate transfer once and preserves its first baseline', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'movie', mediaType: 'movie', title: 'Single Transfer',
      targetUserId: 'quark-user', targetDriveId: 'quark', targetPlatform: 'quark', targetParentFileId: 'quark_root', trackingEnabled: false
    }, 1000)
    const candidate = db.addCandidate(run.id, { kind: 'share', sourcePlatform: 'quark', title: 'Single.Transfer.1080p', locator: 'https://pan.quark.cn/s/single-transfer' }, 1100)!.candidates[0]
    const alternative = db.addCandidate(run.id, { kind: 'share', sourcePlatform: 'quark', title: 'Single.Transfer.2160p', locator: 'https://pan.quark.cn/s/single-transfer-alternative' }, 1150)!.candidates.find(item => item.id !== candidate.id)!
    db.selectCandidate(run.id, candidate.id, '防止重复提交', 1200)
    const afterStaleSelection = db.selectCandidate(run.id, alternative.id, '陈旧 Agent 同时选择另一候选', 1250)!
    expect(afterStaleSelection.candidates).toEqual([
      expect.objectContaining({ id: candidate.id, status: 'selected' }),
      expect.objectContaining({ id: alternative.id, status: 'rejected' })
    ])

    expect(db.claimCandidateTransfer(run.id, candidate.id, 1300)?.status).toBe('transferring')
    expect(db.claimCandidateTransfer(run.id, candidate.id, 1400)).toBeNull()

    db.recordCandidateBaseline(run.id, candidate.id, [{ id: 'before', name: 'before.mkv', path: '/before.mkv', size: 1 }], 1500)
    db.recordCandidateBaseline(run.id, candidate.id, [{ id: 'after', name: 'after.mkv', path: '/after.mkv', size: 2 }], 1600)
    expect(db.getCandidateBaseline(run.id, candidate.id)).toEqual([{ id: 'before', name: 'before.mkv', path: '/before.mkv', size: 1 }])

    db.recordExternalTask(run.id, candidate.id, 'quark-task-1', 'staging-folder', '分享已提交，等待夸克完成', 1700)
    db.beginOrganizing(run.id, '正在整理入库目录', 1800)
    expect(db.completeCandidate(run.id, candidate.id, '入库完成', 1900)?.status).toBe('completed')

    expect(db.selectCandidate(run.id, candidate.id, '陈旧 Agent 回调', 2000)?.status).toBe('completed')
    expect(db.failCandidate(run.id, candidate.id, '夸克网盘容量不足，还需 4.75GB', true, 2100)?.status).toBe('completed')
    expect(db.getRun(run.id)?.events.at(-1)).toMatchObject({ phase: 'finalize', message: '入库完成' })
    db.close()
  })

  it('atomically leases one runnable run to one workflow worker', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'movie', mediaType: 'movie', title: 'Leased Movie',
      targetUserId: 'quark-user', targetDriveId: 'quark', targetPlatform: 'quark', targetParentFileId: 'quark_root', trackingEnabled: false
    }, 1000)

    expect(db.claimRunnableRun('worker-a', run.id, 60_000, 1100)).toMatchObject({ id: run.id })
    expect(db.claimRunnableRun('worker-b', run.id, 60_000, 1100)).toBeNull()
    db.releaseRunClaim(run.id, 'worker-a')
    expect(db.claimRunnableRun('worker-b', run.id, 60_000, 1200)).toMatchObject({ id: run.id })
    db.close()
  })

  it('persists submission intent before the provider task is known', () => {
    const db = createDb()
    const run = db.createRun({ kind: 'movie', mediaType: 'movie', title: 'Intent Movie', targetUserId: 'user-1', targetDriveId: 'drive-1', targetPlatform: 'quark', targetParentFileId: 'root', trackingEnabled: false }, 1000)
    const candidate = db.addCandidate(run.id, { kind: 'share', sourcePlatform: 'quark', title: 'Intent Movie', locator: 'https://pan.quark.cn/s/intent' }, 1100)!.candidates[0]
    db.selectCandidate(run.id, candidate.id, '准备提交', 1200)
    db.claimCandidateTransfer(run.id, candidate.id, 1300)
    db.recordTransferIntent(run.id, candidate.id, '准备调用网盘 API', 1400)
    const afterSubmit = db.recordExternalTask(run.id, candidate.id, 'task-1', 'folder-1', '网盘已接单', 1500)!
    expect(afterSubmit).toMatchObject({ status: 'verifying', candidates: [expect.objectContaining({ externalTaskId: 'task-1', status: 'imported' })] })
    db.close()
  })

  it('falls back to a magnet candidate when share import fails', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'movie', mediaType: 'movie', title: 'Fallback Movie', year: 2026,
      targetUserId: 'pikpak-user', targetDriveId: 'pikpak', targetPlatform: 'pikpak', targetParentFileId: 'pikpak_root', trackingEnabled: false
    }, 1000)
    const withShare = db.addCandidate(run.id, { kind: 'share', sourcePlatform: 'pikpak', title: 'Fallback Movie share', locator: 'https://mypikpak.com/s/share' }, 1100)!
    const share = withShare.candidates[0]
    const withMagnet = db.addCandidate(run.id, { kind: 'magnet', sourcePlatform: 'magnet', title: 'Fallback.Movie.2026.1080p', locator: 'magnet:?xt=urn:btih:fallback' }, 1200)!
    const magnet = withMagnet.candidates.find(candidate => candidate.kind === 'magnet')!

    expect(db.selectCandidate(run.id, share.id, '优先尝试同网盘分享', 1300)?.candidates).toEqual([
      expect.objectContaining({ id: share.id, status: 'selected' }),
      expect.objectContaining({ id: magnet.id, status: 'rejected' })
    ])

    const fallback = db.failCandidate(run.id, share.id, '分享链接已失效', true, 1400)!
    expect(fallback).toMatchObject({ status: 'selecting', phase: 'select', activity: '当前资源失败，正在尝试下一个候选' })
    expect(fallback.candidates).toEqual([
      expect.objectContaining({ id: share.id, status: 'failed', lastError: '分享链接已失效' }),
      expect.objectContaining({ id: magnet.id, status: 'pending' })
    ])
    expect(fallback.events.at(-1)).toMatchObject({ level: 'warning', phase: 'select', message: expect.stringContaining('下一个候选') })
    expect(db.listNotifications()).toEqual([])
    expect(db.listRunnableRuns(10, 1500)).toEqual([expect.objectContaining({ id: run.id, status: 'selecting' })])
    db.close()
  })

  it('remembers terminal dead links and skips them on future tasks', () => {
    const db = createDb()
    const input = {
      kind: 'movie' as const, mediaType: 'movie' as const, title: 'Dead Link Movie',
      targetUserId: 'aliyun-user', targetDriveId: 'aliyun-drive', targetPlatform: 'aliyun', targetParentFileId: 'root', trackingEnabled: false
    }
    const firstRun = db.createRun(input, 1000)
    const firstCandidate = db.addCandidate(firstRun.id, { kind: 'share', sourcePlatform: 'aliyun', title: 'Dead Link Movie', locator: 'https://www.alipan.com/s/dead-share' }, 1100)!.candidates[0]
    db.selectCandidate(firstRun.id, firstCandidate.id, '验证历史失效链接', 1150)
    db.failCandidate(firstRun.id, firstCandidate.id, '分享链接已失效', false, 1200)

    const secondRun = db.createRun({ ...input, title: 'Dead Link Movie 2' }, 1300)
    const afterSkip = db.addCandidate(secondRun.id, { kind: 'share', sourcePlatform: 'aliyun', title: 'Dead Link Movie 2', locator: 'https://www.alipan.com/s/dead-share' }, 1400)!
    expect(afterSkip.candidates).toEqual([])
    expect(afterSkip.events.at(-1)).toMatchObject({ level: 'warning', message: expect.stringContaining('历史失效') })
    db.close()
  })

  it('tries each alternative candidate instead of failing the whole run', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'movie', mediaType: 'movie', title: 'Candidate Chain', targetUserId: 'guangya-user', targetDriveId: 'guangya', targetPlatform: 'guangya', targetParentFileId: 'guangya_root', trackingEnabled: false
    }, 1000)
    const first = db.addCandidate(run.id, { kind: 'share', sourcePlatform: 'guangya', title: 'share A', locator: 'https://guangyapan.com/s/a' }, 1100)!.candidates[0]
    const second = db.addCandidate(run.id, { kind: 'share', sourcePlatform: 'guangya', title: 'share B', locator: 'https://guangyapan.com/s/b' }, 1200)!.candidates.find(candidate => candidate.title === 'share B')!
    const third = db.addCandidate(run.id, { kind: 'magnet', sourcePlatform: 'magnet', title: 'magnet A', locator: 'magnet:?xt=urn:btih:a' }, 1300)!.candidates.find(candidate => candidate.title === 'magnet A')!
    const fourth = db.addCandidate(run.id, { kind: 'magnet', sourcePlatform: 'magnet', title: 'magnet B', locator: 'magnet:?xt=urn:btih:b' }, 1400)!.candidates.find(candidate => candidate.title === 'magnet B')!

    db.selectCandidate(run.id, first.id, 'first', 1500)
    const afterFirst = db.failCandidate(run.id, first.id, 'dead share', true, 1600)!
    expect(afterFirst.candidates.filter(candidate => candidate.status === 'pending').map(candidate => candidate.id)).toEqual([second.id, third.id, fourth.id])

    db.selectCandidate(run.id, second.id, 'second', 1700)
    const afterSecond = db.failCandidate(run.id, second.id, 'dead share', true, 1800)!
    expect(afterSecond.candidates.filter(candidate => candidate.status === 'pending').map(candidate => candidate.id)).toEqual([third.id, fourth.id])

    db.selectCandidate(run.id, third.id, 'third', 1900)
    const afterThird = db.failCandidate(run.id, third.id, 'dead magnet', true, 2000)!
    expect(afterThird.candidates.filter(candidate => candidate.status === 'pending').map(candidate => candidate.id)).toEqual([fourth.id])
    expect(afterThird.status).toBe('selecting')
    db.close()
  })

  it('does not fall back when the target drive has no offline capability', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'movie', mediaType: 'movie', title: 'Share Only Movie', targetUserId: 'aliyun-user', targetDriveId: 'aliyun-drive', targetPlatform: 'aliyun', targetParentFileId: 'root', trackingEnabled: false
    }, 1000)
    const share = db.addCandidate(run.id, { kind: 'share', sourcePlatform: 'aliyun', title: 'Share Only Movie', locator: 'https://www.alipan.com/s/share-only' }, 1100)!.candidates[0]
    db.addCandidate(run.id, { kind: 'magnet', sourcePlatform: 'magnet', title: 'Share.Only.Movie', locator: 'magnet:?xt=urn:btih:not-supported' }, 1200)
    db.selectCandidate(run.id, share.id, '优先分享', 1300)

    const failed = db.failCandidate(run.id, share.id, '分享失效', false, 1400)!
    expect(failed.status).toBe('failed')
    expect(failed.candidates.find(candidate => candidate.kind === 'magnet')?.status).toBe('rejected')
    expect(db.listNotifications()).toEqual([expect.objectContaining({ status: 'failed', message: '分享失效' })])
    db.close()
  })

  it('allows completed media to create independent missing-season runs', () => {
    const db = createDb()
    const base = {
      tmdbId: 100, mediaType: 'tv' as const, title: '示例剧集', year: 2026,
      targetUserId: 'user-115', targetDriveId: 'drive-115', targetPlatform: '115', targetParentFileId: 'shows', trackingEnabled: true
    }
    const original = db.createRun({ ...base, kind: 'season', seasonNumber: 1 }, 1000)
    const candidate = db.addCandidate(original.id, { kind: 'magnet', sourcePlatform: 'magnet', title: 'S01', locator: 'magnet:?xt=urn:btih:season1' }, 1100)!.candidates[0]
    db.selectCandidate(original.id, candidate.id, '匹配第一季', 1150)
    db.claimCandidateTransfer(original.id, candidate.id, 1175)
    db.beginVerifyingCandidate(original.id, candidate.id, '核对目录', 1200)
    db.beginOrganizing(original.id, undefined, 1300)
    db.completeCandidate(original.id, candidate.id, '入库完成', 1400)

    const season1 = db.createRun({ ...base, kind: 'missing', seasonNumber: 1, missingEpisodes: [4, 5] }, 1500)
    const season2 = db.createRun({ ...base, kind: 'missing', seasonNumber: 2, missingEpisodes: [1, 2] }, 1600)
    expect(season1.target.missingEpisodes).toEqual([4, 5])
    expect(season2.target.seasonNumber).toBe(2)
    expect(() => db.createRun({ ...base, kind: 'missing', seasonNumber: 1, missingEpisodes: [4, 5] }, 1700)).toThrow('进行中的获取任务')
    db.close()
  })

  it('isolates active missing-season tasks by cloud-drive account and target directory', () => {
    const db = createDb()
    const base = {
      kind: 'missing' as const, tmdbId: 301, mediaType: 'tv' as const, title: 'Multi Drive Show', seasonNumber: 1, missingEpisodes: [2], trackingEnabled: true
    }
    db.createRun({ ...base, targetUserId: 'user-a', targetDriveId: 'drive-a', targetPlatform: '115', targetParentFileId: 'shows-a' }, 1000)
    expect(db.createRun({ ...base, targetUserId: 'user-a', targetDriveId: 'drive-a', targetPlatform: '115', targetParentFileId: 'shows-b' }, 1100)).toMatchObject({ status: 'queued' })
    expect(() => db.createRun({ ...base, targetUserId: 'user-a', targetDriveId: 'drive-a', targetPlatform: '115', targetParentFileId: 'shows-b' }, 1150)).toThrow('进行中的获取任务')
    expect(db.createRun({ ...base, targetUserId: 'user-b', targetDriveId: 'drive-b', targetPlatform: 'cloud123', targetParentFileId: 'shows-b' }, 1200)).toMatchObject({ status: 'queued' })
    db.close()
  })

  it('persists a season subscription with aired and obtained episode state', () => {
    const db = createDb()
    const tracking = db.createTracking({
      mediaLibraryItemId: 'library-tv', tmdbId: 100, mediaType: 'tv', title: '示例剧集', year: 2026, seasonNumber: 2,
      targetUserId: 'user-115', targetDriveId: 'drive-115', targetPlatform: '115', targetParentFileId: 'shows'
    }, 1000)
    expect(tracking).toMatchObject({ seasonNumber: 2, latestAiredEpisode: 0, obtainedEpisodeNumbers: [], status: 'tracking' })

    const updated = db.upsertTracking(tracking.targetId, 2, 10, 6, [1, 2, 3, 5, 6], [4], 'tracking', 3000, 2000)
    expect(updated).toMatchObject({ totalEpisodes: 10, latestAiredEpisode: 6, obtainedEpisodes: 5, obtainedEpisodeNumbers: [1, 2, 3, 5, 6], providerAheadEpisodes: [], missingEpisodes: [4], nextCheckAt: 3000 })
    expect(db.getTarget(tracking.targetId)).toMatchObject({ targetParentFileId: 'shows', trackingEnabled: true })
    const patrol = db.createRun({
      existingTargetId: tracking.targetId, kind: 'patrol', tmdbId: 100, mediaType: 'tv', title: '示例剧集', year: 2026, seasonNumber: 2, missingEpisodes: [4],
      targetUserId: 'user-115', targetDriveId: 'drive-115', targetPlatform: '115', targetParentFileId: 'shows', trackingEnabled: true
    }, 3500)
    expect(patrol.target).toMatchObject({ id: tracking.targetId, missingEpisodes: [4] })
    expect(db.listTracking()).toHaveLength(1)
    expect(db.endTracking(tracking.id, 4000)).toMatchObject({ status: 'ended', nextCheckAt: undefined })
    db.close()
  })

  it('persists partial completion as a terminal notification', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'season', tmdbId: 200, mediaType: 'tv', title: 'Partial Show', seasonNumber: 1,
      targetUserId: 'user-115', targetDriveId: 'drive-115', targetPlatform: '115', targetParentFileId: 'shows', trackingEnabled: true
    }, 1000)
    const candidate = db.addCandidate(run.id, { kind: 'magnet', sourcePlatform: 'magnet', title: 'Partial.Show.S01E01', locator: 'magnet:?xt=urn:btih:partial' }, 1100)!.candidates[0]
    db.selectCandidate(run.id, candidate.id, '匹配缺集', 1125)
    db.claimCandidateTransfer(run.id, candidate.id, 1150)
    db.beginVerifyingCandidate(run.id, candidate.id, '核对入库目录', 1175)
    const partial = db.partialCandidate(run.id, candidate.id, '已导入部分内容，但仍缺 E02', 1200)!
    expect(partial).toMatchObject({ status: 'partial', finishedAt: 1200, progress: 100 })
    expect(db.listNotifications()).toEqual([expect.objectContaining({ status: 'partial', message: '已导入部分内容，但仍缺 E02' })])
    db.close()
  })

  it('continues with remaining candidates after a partial import', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'season', tmdbId: 201, mediaType: 'tv', title: 'Continue Show', seasonNumber: 1, missingEpisodes: [1, 2, 3],
      targetUserId: 'user-115', targetDriveId: 'drive-115', targetPlatform: '115', targetParentFileId: 'shows', trackingEnabled: true
    }, 1000)
    const first = db.addCandidate(run.id, { kind: 'magnet', sourcePlatform: 'magnet', title: 'Continue.Show.S01E01', locator: 'magnet:?xt=urn:btih:first' }, 1100)!.candidates[0]
    const second = db.addCandidate(run.id, { kind: 'magnet', sourcePlatform: 'magnet', title: 'Continue.Show.S01E02-E03', locator: 'magnet:?xt=urn:btih:second' }, 1200)!.candidates.find(candidate => candidate.title.includes('E02'))!
    db.selectCandidate(run.id, first.id, '先导入 E01', 1300)
    db.claimCandidateTransfer(run.id, first.id, 1325)
    db.beginVerifyingCandidate(run.id, first.id, '核对 E01', 1350)

    const continued = db.continueAfterPartialCandidate(run.id, first.id, [{ seasonNumber: 1, missingEpisodes: [2, 3] }], '已导入 E01，继续补齐 E02、E03', 1400)!
    expect(continued).toMatchObject({ status: 'selecting', phase: 'select', activity: '已部分入库，继续补齐缺集' })
    expect(continued.target.missingEpisodes).toEqual([2, 3])
    expect(continued.candidates).toEqual([
      expect.objectContaining({ id: first.id, status: 'imported' }),
      expect.objectContaining({ id: second.id, status: 'pending' })
    ])
    expect(db.listNotifications()).toEqual([])
    db.close()
  })

  it('revives a no-coverage run when the user adds a candidate manually', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'movie', mediaType: 'movie', title: 'Manual Candidate Movie',
      targetUserId: 'aliyun-user', targetDriveId: 'aliyun-drive', targetPlatform: 'aliyun', targetParentFileId: 'root', trackingEnabled: false
    }, 1000)
    const noCoverage = db.markNoCoverage(run.id, '暂未找到可导入资源', 1100)!
    expect(noCoverage).toMatchObject({ status: 'no_coverage', finishedAt: 1100 })

    const revived = db.addCandidate(run.id, {
      kind: 'share', sourcePlatform: 'aliyun', title: 'Manual Candidate Movie 1080p', locator: 'https://www.alipan.com/s/manual-candidate'
    }, 1200)!
    expect(revived).toMatchObject({ status: 'selecting', phase: 'select', finishedAt: undefined, errorMessage: undefined })
    expect(revived.candidates).toEqual([expect.objectContaining({ status: 'pending' })])
    expect(db.listRunnableRuns(10, 1300)).toEqual([expect.objectContaining({ id: run.id, status: 'selecting' })])
    db.close()
  })

  it('revives a failed run when the user adds a better candidate manually', () => {
    const db = createDb()
    const run = db.createRun({
      kind: 'movie', mediaType: 'movie', title: 'Recover Failed Movie',
      targetUserId: 'aliyun-user', targetDriveId: 'aliyun-drive', targetPlatform: 'aliyun', targetParentFileId: 'root', trackingEnabled: false
    }, 1000)
    const stale = db.addCandidate(run.id, { kind: 'share', sourcePlatform: 'aliyun', title: 'Recover Failed Movie old', locator: 'https://www.alipan.com/s/stale' }, 1100)!.candidates[0]
    db.selectCandidate(run.id, stale.id, '候选待验证', 1150)
    const failed = db.failCandidate(run.id, stale.id, '分享链接已失效', false, 1200)!
    expect(failed).toMatchObject({ status: 'failed', finishedAt: 1200, errorMessage: '分享链接已失效' })

    const revived = db.addCandidate(run.id, { kind: 'share', sourcePlatform: 'aliyun', title: 'Recover Failed Movie new', locator: 'https://www.alipan.com/s/fresh' }, 1300)!
    expect(revived).toMatchObject({ status: 'selecting', phase: 'select', finishedAt: undefined, errorMessage: undefined, attemptCount: 0, nextAttemptAt: undefined })
    expect(revived.candidates).toEqual([
      expect.objectContaining({ id: stale.id, status: 'failed', lastError: '分享链接已失效' }),
      expect.objectContaining({ title: 'Recover Failed Movie new', status: 'pending' })
    ])
    expect(db.listRunnableRuns(10, 1400)).toEqual([expect.objectContaining({ id: run.id, status: 'selecting' })])
    db.close()
  })
})
