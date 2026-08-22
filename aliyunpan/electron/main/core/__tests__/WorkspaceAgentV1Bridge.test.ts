import { describe, expect, it } from 'vitest'
import { isWorkspaceAgentV1ExecutableAction, workspaceAgentScope, workspacePlanActionToAgentAction } from '../../workspaceAgent/WorkspaceAgentV1Bridge.ts'

const task = {
  id: 'task-1',
  kind: 'import_share' as const,
  scope: { userId: 'user-1', driveId: 'drive-1', rootId: 'root', platform: 'aliyun', name: '我的文件' }
}

describe('WorkspaceAgentV1Bridge', () => {
  it('maps only the exact workspace capability needed by each plan kind', () => {
    expect(workspaceAgentScope({ ...task, kind: 'organize_files' })).toMatchObject({ operations: ['files.list', 'files.move'] })
    expect(workspaceAgentScope({ ...task, kind: 'cleanup_duplicates' })).toMatchObject({ operations: ['files.list', 'trash.move'] })
    expect(workspaceAgentScope(task)).toMatchObject({ operations: ['files.list', 'share.import'] })
  })

  it('copies share identifiers into a frozen plan without copying the share URL', () => {
    const action = workspacePlanActionToAgentAction(task as any, { id: 'a-1', kind: 'import_share', label: '导入分享', share: { url: 'https://example.com/s/secret', shareId: 'share-1', fileIds: ['file-1'] } }, { id: 'handle-1' } as any)

    expect(action).toMatchObject({ kind: 'import_share', resourceHandleIds: ['handle-1'], parameters: { shareId: 'share-1', fileIds: ['file-1'] } })
    expect(JSON.stringify(action)).not.toContain('https://example.com')
  })

  it('limits V1 cutover writes to the executor capabilities it actually implements', () => {
    expect(isWorkspaceAgentV1ExecutableAction({ kind: 'move' })).toBe(true)
    expect(isWorkspaceAgentV1ExecutableAction({ kind: 'trash' })).toBe(true)
    expect(isWorkspaceAgentV1ExecutableAction({ kind: 'download' })).toBe(false)
    expect(isWorkspaceAgentV1ExecutableAction({ kind: 'import_share' })).toBe(false)
  })
})
