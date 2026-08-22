import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import type { MediaAcquisitionRunView } from '@shared/types/mediaAcquisition'
import { mediaAcquisitionCandidateEvidence, mediaAcquisitionScope, mediaAcquisitionTransferAction, mediaAcquisitionTransferScope } from '../../mediaAcquisition/MediaAcquisitionV1Bridge'

const run = {
  id: 'run-1',
  targetId: 'target-1',
  kind: 'movie',
  status: 'selecting',
  phase: 'select',
  progress: 35,
  activity: '正在选择资源',
  attemptCount: 1,
  searchAttemptCount: 1,
  startedAt: 1,
  target: {
    id: 'target-1', mediaKey: 'movie:1', mediaType: 'movie', title: '测试电影',
    targetUserId: 'user-1', targetDriveId: 'drive-1', targetPlatform: 'aliyun', targetParentFileId: 'folder-1', fetchSubtitles: true, trackingEnabled: false, createdAt: 1
  },
  events: [],
  candidates: [{ id: 'candidate-1', runId: 'run-1', kind: 'share', sourcePlatform: 'aliyun', title: '测试电影 4K', detail: '简体字幕', status: 'pending', createdAt: 2 }]
} satisfies MediaAcquisitionRunView

describe('MediaAcquisitionV1Bridge', () => {
  it('仅镜像获取证据，并且不泄露候选链接或提取码', () => {
    expect(mediaAcquisitionScope(run)).toEqual({ accountId: 'user-1', driveId: 'drive-1', rootId: 'folder-1', platform: 'aliyun', operations: ['files.list'] })
    expect(mediaAcquisitionCandidateEvidence(run, run.candidates[0])).toEqual({
      source: 'media_candidate',
      summary: '候选资源：测试电影 4K',
      data: { candidateId: 'candidate-1', kind: 'share', sourcePlatform: 'aliyun', title: '测试电影 4K', status: 'pending' },
      externalRef: 'media-candidate:candidate-1:pending:2'
    })
  })

  it('将候选转存限制为一个已取证的分享或离线下载操作', () => {
    expect(mediaAcquisitionTransferScope(run, run.candidates[0])).toEqual({ accountId: 'user-1', driveId: 'drive-1', rootId: 'folder-1', platform: 'aliyun', operations: ['files.list', 'share.import'] })
    expect(mediaAcquisitionTransferAction(run.candidates[0], 'handle-1')).toEqual({
      id: 'media-transfer:candidate-1',
      kind: 'import_share',
      label: '导入分享候选：测试电影 4K',
      idempotencyKey: 'media-transfer:candidate-1',
      resourceHandleIds: ['handle-1'],
      parameters: { candidateId: 'candidate-1', candidateKind: 'share' }
    })
    const magnet = { ...run.candidates[0], id: 'candidate-2', kind: 'magnet' as const }
    expect(mediaAcquisitionTransferScope(run, magnet).operations).toEqual(['files.list', 'offline.create'])
    expect(mediaAcquisitionTransferAction(magnet, 'handle-2').kind).toBe('offline_download')
  })

  it('keeps provider submission and polling out of the renderer executors', () => {
    const root = process.cwd()
    const sources = ['magnetExecutor.ts', 'shareExecutor.ts', 'externalUrlExecutor.ts', 'subtitleExecutor.ts', 'workflowRunner.ts'].map(file => readFileSync(join(root, 'src/services/mediaAcquisition', file), 'utf8')).join('\n')
    expect(sources).not.toMatch(/api(?:Drive115|Cloud123|Guangya|PikPak)Offline(?:Create|Process)|api(?:Quark|Guangya|PikPak)SaveShareFilesBatch|ApiSaveShareFilesBatch/)
    expect(readFileSync(join(root, 'src/services/mediaAcquisition/client.ts'), 'utf8')).not.toContain('mediaAcquisition:getCandidateLocator')
    expect(readFileSync(join(root, 'electron/main/mediaAcquisition/MediaProviderAdapter.ts'), 'utf8')).toContain('getMediaAcquisitionProviderTransferStatus')
  })
})
