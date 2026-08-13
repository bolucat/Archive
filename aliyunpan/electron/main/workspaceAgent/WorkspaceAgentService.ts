import { app } from 'electron'
import { join } from 'path'
import type { CreateWorkspacePlanInput, CreateWorkspaceTaskInput, UpdateWorkspacePlanSelectionInput, WorkspaceExecutionEvent, WorkspaceTaskView } from '@shared/types/workspaceAgent'
import { WorkspaceAgentDb } from './WorkspaceAgentDb'

let db: WorkspaceAgentDb | null = null
function getDb(): WorkspaceAgentDb {
  if (!db) db = new WorkspaceAgentDb(join(app.getPath('userData'), 'workspace-agent.db'))
  return db
}

export function createWorkspaceTask(input: CreateWorkspaceTaskInput): WorkspaceTaskView {
  if (!input.goal?.trim()) throw new Error('请输入要处理的网盘目标')
  if (!input.scope?.userId || !input.scope.driveId || !input.scope.rootId) throw new Error('请先选择一个网盘与根目录')
  return getDb().createTask(input)
}
export function listWorkspaceTasks(limit?: number, includeArchived?: boolean): WorkspaceTaskView[] { return getDb().listTasks(Math.min(Math.max(limit || 50, 1), 100), !!includeArchived) }
export function getWorkspaceTask(id: string): WorkspaceTaskView | null { return getDb().getTask(id) }
export function addWorkspaceEvidence(taskId: string, source: string, summary: string, data: Record<string, unknown>): WorkspaceTaskView | null { getDb().addEvidence(taskId, source, summary, data); return getDb().getTask(taskId) }
export function saveWorkspacePlan(input: CreateWorkspacePlanInput): WorkspaceTaskView | null { getDb().savePlan(input); return getDb().getTask(input.taskId) }
export function approveWorkspacePlan(taskId: string, hash: string): WorkspaceTaskView { return getDb().approvePlan(taskId, hash) }
export function updateWorkspacePlanSelection(input: UpdateWorkspacePlanSelectionInput): WorkspaceTaskView { return getDb().updatePlanSelection(input) }
export function rejectWorkspacePlan(taskId: string, hash: string): WorkspaceTaskView { return getDb().rejectPlan(taskId, hash) }
export function completeWorkspaceTask(taskId: string, status: 'completed' | 'partial' | 'failed' | 'stale', message: string): WorkspaceTaskView { return getDb().complete(taskId, status, message) }
export function cancelWorkspaceTask(taskId: string): WorkspaceTaskView { return getDb().cancel(taskId) }
export function resumeWorkspaceTask(taskId: string): WorkspaceTaskView { return getDb().resume(taskId) }
export function archiveWorkspaceTask(taskId: string): WorkspaceTaskView { return getDb().archiveTask(taskId) }
export function restoreWorkspaceTask(taskId: string): WorkspaceTaskView { return getDb().restoreTask(taskId) }
export function addWorkspaceEvent(taskId: string, level: WorkspaceExecutionEvent['level'], message: string, data?: Record<string, unknown>): WorkspaceTaskView | null { getDb().addEvent(taskId, level, message, data); return getDb().getTask(taskId) }
export function destroyWorkspaceAgentDb(): void { db?.close(); db = null }
