import { describe, expect, it } from 'vitest'
import { preflightAgentCliAction } from '../../agent/AgentCliPreflight.ts'

const snapshot = { accountId: 'user-1', driveId: 'drive-1', resourceId: 'file-1', parentResourceId: 'folder-1', name: '报告.pdf' }
const action = { id: 'move-1', kind: 'move' as const, label: '移动：报告.pdf', idempotencyKey: 'move:user-1:file-1', resourceHandleIds: ['handle-1'], parameters: { targetParentFileId: 'folder-2' } }
const workflow: any = {
  id: 'workflow-1',
  scope: { accountId: 'user-1', driveId: 'drive-1', rootId: 'root', platform: 'aliyun', operations: ['files.list', 'files.move'] },
  plan: { id: 'plan-1', status: 'approved' },
  resourceHandles: [{ id: 'handle-1', snapshot }]
}

describe('AgentCliPreflight', () => {
  it('rechecks source and destination before materializing a bounded CLI move plan', async () => {
    const inspected = new Map([
      ['file-1', { kind: 'file' as const, snapshot }],
      ['folder-2', { kind: 'folder' as const, snapshot: { accountId: 'user-1', driveId: 'drive-1', resourceId: 'folder-2', name: '已整理' } }]
    ])
    const result = await preflightAgentCliAction(workflow, action, { inspectResource: async (_scope, id) => inspected.get(id)!, isWithinScopeRoot: async () => true } as any, 0)

    expect(result).toMatchObject({ command: 'files move-apply', plan: { provider: 'aliyun', account_id: 'aliyun_user-1', items: [{ file_id: 'file-1', from_parent_file_id: 'folder-1', to_parent_file_id: 'folder-2' }] } })
  })

  it('refuses a changed resource or a target outside the approved root', async () => {
    await expect(preflightAgentCliAction(workflow, action, {
      inspectResource: async (_scope, id) => id === 'file-1' ? { kind: 'file', snapshot: { ...snapshot, name: '已重命名.pdf' } } : { kind: 'folder', snapshot: { ...snapshot, resourceId: id, name: '已整理' } },
      isWithinScopeRoot: async () => true
    } as any)).rejects.toThrow('资源名称已变化')

    await expect(preflightAgentCliAction(workflow, action, {
      inspectResource: async (_scope, id) => id === 'file-1' ? { kind: 'file', snapshot } : { kind: 'folder', snapshot: { ...snapshot, resourceId: id, name: '已整理' } },
      isWithinScopeRoot: async (_scope, id) => id !== 'folder-2'
    } as any)).rejects.toThrow('移动目标不在授权根目录内')
  })

  it('never materializes a pending plan before the user approves its exact hash', async () => {
    await expect(preflightAgentCliAction({ ...workflow, plan: { ...workflow.plan, status: 'awaiting_approval' } }, action, {
      inspectResource: async () => ({ kind: 'file', snapshot }),
      isWithinScopeRoot: async () => true
    } as any)).rejects.toThrow('只能预检已批准的 Agent 计划')
  })
})
