import { afterEach, describe, expect, it } from 'vitest'
import { mkdtempSync, rmSync } from 'fs'
import { join } from 'path'
import { tmpdir } from 'os'
import { WorkspaceAgentDb } from '../../workspaceAgent/WorkspaceAgentDb.ts'

const dirs: string[] = []
function createDb() {
  const dir = mkdtempSync(join(tmpdir(), 'boxplayer-workspace-agent-'))
  dirs.push(dir)
  return new WorkspaceAgentDb(join(dir, 'workspace-agent.db'))
}
const input = { goal: '清理重复文件', kind: 'cleanup_duplicates' as const, scope: { userId: 'user-1', driveId: 'drive-1', platform: 'aliyun', rootId: 'root', name: '测试网盘' } }

afterEach(() => { while (dirs.length) rmSync(dirs.pop()!, { recursive: true, force: true }) })

describe('WorkspaceAgentDb', () => {
  it('requires a frozen complete plan before it permits execution', () => {
    const db = createDb()
    const task = db.createTask(input, 1000)
    const evidence = db.addEvidence(task.id, 'scanDriveDuplicates', '找到一组重复文件', { files: [{ fileId: 'a' }] }, 1100)
    const plan = db.savePlan({ taskId: task.id, title: '重复文件清理计划', summary: '移入回收站', risk: '可恢复', evidenceIds: [evidence.id], actions: [{ id: 'action-1', kind: 'trash', label: '移入回收站：a', snapshot: { userId: 'user-1', driveId: 'drive-1', fileId: 'a', name: 'a' } }] }, 1200)
    expect(db.getTask(task.id)).toMatchObject({ status: 'awaiting_approval', plan: { hash: plan.hash, status: 'awaiting_approval' } })
    expect(() => db.approvePlan(task.id, 'changed', 1300)).toThrow('计划内容已变化')
    const approved = db.approvePlan(task.id, plan.hash, 1300)
    expect(approved).toMatchObject({ status: 'executing', approval: { status: 'approved', planHash: plan.hash } })
    expect(db.complete(task.id, 'completed', '完成', 1400)).toMatchObject({ status: 'completed', plan: { status: 'completed' } })
    db.close()
  })

  it('pauses in-flight work after a new application session and invalidates resumed plans', () => {
    const dir = mkdtempSync(join(tmpdir(), 'boxplayer-workspace-agent-recover-'))
    dirs.push(dir)
    const path = join(dir, 'workspace-agent.db')
    const first = new WorkspaceAgentDb(path)
    const task = first.createTask(input, 1000)
    first.addEvidence(task.id, 'scan', '完成', {}, 1100)
    first.savePlan({ taskId: task.id, title: '计划', summary: '摘要', risk: '风险', evidenceIds: [], actions: [{ id: 'a', kind: 'trash', label: '删除', snapshot: { userId: 'user-1', driveId: 'drive-1', fileId: 'file', name: 'file' } }] }, 1200)
    first.close()

    const restarted = new WorkspaceAgentDb(path)
    expect(restarted.getTask(task.id)).toMatchObject({ status: 'paused' })
    expect(restarted.resume(task.id, 1400)).toMatchObject({ status: 'planning', plan: { status: 'stale' } })
    restarted.close()
  })

  it('re-signs a plan with only the user-selected actions before approval', () => {
    const db = createDb()
    const task = db.createTask(input, 1000)
    const evidence = db.addEvidence(task.id, 'scan', '完成', {}, 1100)
    const plan = db.savePlan({ taskId: task.id, title: '大文件清理计划', summary: '移入回收站', risk: '可恢复', evidenceIds: [evidence.id], actions: [
      { id: 'a', kind: 'trash', label: '移入回收站：a', snapshot: { userId: 'user-1', driveId: 'drive-1', fileId: 'a', name: 'a' } },
      { id: 'b', kind: 'trash', label: '移入回收站：b', snapshot: { userId: 'user-1', driveId: 'drive-1', fileId: 'b', name: 'b' } }
    ] }, 1200)
    const selected = db.updatePlanSelection({ taskId: task.id, planHash: plan.hash, actionIds: ['b'] }, 1300)
    expect(selected.plan).toMatchObject({ actions: [{ id: 'b' }] })
    expect(selected.plan!.hash).not.toBe(plan.hash)
    expect(db.approvePlan(task.id, selected.plan!.hash, 1400)).toMatchObject({ status: 'executing', plan: { actions: [{ id: 'b' }] } })
    db.close()
  })
})
