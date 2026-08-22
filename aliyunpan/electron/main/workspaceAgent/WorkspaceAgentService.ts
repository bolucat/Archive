import { app } from 'electron'
import { join } from 'path'
import type { CreateWorkspacePlanInput, CreateWorkspaceTaskInput, UpdateWorkspacePlanSelectionInput, WorkspaceExecutionEvent, WorkspaceTaskView } from '@shared/types/workspaceAgent'
import { WorkspaceAgentDb } from './WorkspaceAgentDb'
import { getWorkspaceAgentV1Audit, mirrorWorkspaceApproval, mirrorWorkspaceEvidence, mirrorWorkspacePlan, mirrorWorkspaceTask, mirrorWorkspaceTerminal, type WorkspaceAgentV1Audit } from './WorkspaceAgentV1Bridge'

let db: WorkspaceAgentDb | null = null
function getDb(): WorkspaceAgentDb {
  if (!db) db = new WorkspaceAgentDb(join(app.getPath('userData'), 'workspace-agent.db'))
  return db
}

export function createWorkspaceTask(input: CreateWorkspaceTaskInput): WorkspaceTaskView {
  if (!input.goal?.trim()) throw new Error('请输入要处理的网盘目标')
  if (!input.scope?.userId || !input.scope.driveId || !input.scope.rootId) throw new Error('请先选择一个网盘与根目录')
  const task = getDb().createTask(input)
  mirror('创建工作流', () => mirrorWorkspaceTask(task))
  return task
}
export function listWorkspaceTasks(limit?: number, includeArchived?: boolean): WorkspaceTaskView[] { return getDb().listTasks(Math.min(Math.max(limit || 50, 1), 100), !!includeArchived) }
export function getWorkspaceTask(id: string): WorkspaceTaskView | null { return getDb().getTask(id) }
export function getWorkspaceTaskV1Audit(taskId: string): WorkspaceAgentV1Audit | null {
  const task = getDb().getTask(taskId)
  if (task) mirror('补偿同步终态', () => mirrorWorkspaceTerminal(task))
  return getWorkspaceAgentV1Audit(taskId)
}
export function addWorkspaceEvidence(taskId: string, source: string, summary: string, data: Record<string, unknown>): WorkspaceTaskView | null {
  getDb().addEvidence(taskId, source, summary, data)
  const task = getDb().getTask(taskId)
  if (task) mirror('同步取证', () => mirrorWorkspaceEvidence(task))
  return task
}
export function saveWorkspacePlan(input: CreateWorkspacePlanInput): WorkspaceTaskView | null {
  getDb().savePlan(input)
  const task = getDb().getTask(input.taskId)
  if (task) mirror('同步计划', () => mirrorWorkspacePlan(task))
  return task
}
export function approveWorkspacePlan(taskId: string, hash: string): WorkspaceTaskView {
  const task = getDb().approvePlan(taskId, hash)
  mirror('同步审批', () => mirrorWorkspaceApproval(task))
  return task
}
export function updateWorkspacePlanSelection(input: UpdateWorkspacePlanSelectionInput): WorkspaceTaskView {
  const task = getDb().updatePlanSelection(input)
  mirror('同步计划选择', () => mirrorWorkspacePlan(task))
  return task
}
export function rejectWorkspacePlan(taskId: string, hash: string): WorkspaceTaskView { return getDb().rejectPlan(taskId, hash) }
export function completeWorkspaceTask(taskId: string, status: 'completed' | 'partial' | 'failed' | 'stale', message: string): WorkspaceTaskView {
  const task = getDb().complete(taskId, status, message)
  mirror('同步终态', () => mirrorWorkspaceTerminal(task))
  return task
}
export function cancelWorkspaceTask(taskId: string): WorkspaceTaskView {
  const task = getDb().cancel(taskId)
  mirror('同步取消', () => mirrorWorkspaceTerminal(task))
  return task
}
export function resumeWorkspaceTask(taskId: string): WorkspaceTaskView { return getDb().resume(taskId) }
export function archiveWorkspaceTask(taskId: string): WorkspaceTaskView { return getDb().archiveTask(taskId) }
export function restoreWorkspaceTask(taskId: string): WorkspaceTaskView { return getDb().restoreTask(taskId) }
export function addWorkspaceEvent(taskId: string, level: WorkspaceExecutionEvent['level'], message: string, data?: Record<string, unknown>): WorkspaceTaskView | null { getDb().addEvent(taskId, level, message, data); return getDb().getTask(taskId) }
export function destroyWorkspaceAgentDb(): void { db?.close(); db = null }

function mirror(stage: string, work: () => void): void {
  try { work() } catch (error: any) { console.warn(`[Agent V1 workspace shadow] ${stage}失败：${error?.message || error}`) }
}
