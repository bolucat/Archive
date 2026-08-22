import { describe, expect, it } from 'vitest'
import { aiSearchEvidence, aiSearchScope, aiSearchWriteAction, aiSearchWriteScope } from '../../aiSearch/AiSearchV1Bridge'

describe('AiSearchV1Bridge', () => {
  it('mirrors an explicitly selected account as read-only evidence', () => {
    expect(aiSearchScope({ userId: 'user-1', driveId: 'drive-1', rootId: 'resource_root', platform: 'aliyun' })).toEqual({ accountId: 'user-1', driveId: 'drive-1', rootId: 'resource_root', platform: 'aliyun', operations: ['files.list'] })
  })

  it('keeps tool evidence useful without persisting signed URLs or passwords', () => {
    const evidence = aiSearchEvidence({ sessionId: 'session-1', runId: 'run-1', toolCallId: 'call-1', toolName: 'searchMyFiles', isError: false, result: { total: 3, files: [{ name: 'private.pdf', url: 'https://example.com/?token=secret' }], password: '1234' } })

    expect(evidence).toMatchObject({ source: 'ai_search.searchMyFiles', summary: 'searchMyFiles 已完成', data: { toolName: 'searchMyFiles', isError: false, total: 3 }, externalRef: 'ai-search:session-1:run-1:tool:call-1' })
    expect(JSON.stringify(evidence)).not.toContain('https://example.com')
    expect(JSON.stringify(evidence)).not.toContain('1234')
    expect(JSON.stringify(evidence)).not.toContain('private.pdf')
  })

  it('freezes each confirmed write against one re-inspected resource handle', () => {
    const target = { userId: 'user-1', driveId: 'drive-1', rootId: 'root', platform: 'aliyun' }
    const action = aiSearchWriteAction({ confirmationId: 'thread:message:0', kind: 'move', target, fileIds: ['file-1'], targetParentFileId: 'folder-1' }, { accountId: 'user-1', driveId: 'drive-1', resourceId: 'file-1', parentResourceId: 'old-parent', name: 'report.pdf' }, 'handle-1')

    expect(aiSearchWriteScope(target, 'move')).toEqual({ ...aiSearchScope(target), operations: ['files.list', 'files.move'] })
    expect(action).toMatchObject({ kind: 'move', resourceHandleIds: ['handle-1'], parameters: { targetParentFileId: 'folder-1' }, idempotencyKey: 'ai-search:thread:message:0:file-1' })
  })
})
