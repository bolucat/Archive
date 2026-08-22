import type { CreateWorkspacePlanInput, CreateWorkspaceTaskInput, UpdateWorkspacePlanSelectionInput, WorkspaceExecutionEvent, WorkspaceTaskView } from '@shared/types/workspaceAgent'

export type WorkspaceAgentV1Audit = { cutover: boolean; workflowId: string; workflowStatus: string; plan?: { hash: string; status: string }; grant?: { status: string; expiresAt: number } }

function invoke<T>(channel: string, ...args: unknown[]): Promise<T> {
  const ipc = window.Electron?.ipcRenderer
  if (!ipc) return Promise.reject(new Error('AI 工作台仅支持桌面客户端'))
  return ipc.invoke(channel, ...args) as Promise<T>
}
const payload = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T

export function createWorkspaceTask(input: CreateWorkspaceTaskInput): Promise<WorkspaceTaskView> { return invoke('workspaceAgent:create', payload(input)) }
export function listWorkspaceTasks(limit?: number, includeArchived?: boolean): Promise<WorkspaceTaskView[]> { return invoke('workspaceAgent:list', limit, includeArchived) }
export function getWorkspaceTask(id: string): Promise<WorkspaceTaskView | null> { return invoke('workspaceAgent:get', id) }
export function getWorkspaceTaskV1Audit(id: string): Promise<WorkspaceAgentV1Audit | null> { return invoke('workspaceAgent:getV1Audit', id) }
export function addWorkspaceEvidence(taskId: string, source: string, summary: string, data: Record<string, unknown>): Promise<WorkspaceTaskView | null> { return invoke('workspaceAgent:addEvidence', taskId, source, summary, payload(data)) }
export function saveWorkspacePlan(input: CreateWorkspacePlanInput): Promise<WorkspaceTaskView | null> { return invoke('workspaceAgent:savePlan', payload(input)) }
export function approveWorkspacePlan(taskId: string, hash: string): Promise<WorkspaceTaskView> { return invoke('workspaceAgent:approve', taskId, hash) }
export function updateWorkspacePlanSelection(input: UpdateWorkspacePlanSelectionInput): Promise<WorkspaceTaskView> { return invoke('workspaceAgent:updateSelection', payload(input)) }
export function rejectWorkspacePlan(taskId: string, hash: string): Promise<WorkspaceTaskView> { return invoke('workspaceAgent:reject', taskId, hash) }
export function completeWorkspaceTask(taskId: string, status: 'completed' | 'partial' | 'failed' | 'stale', message: string): Promise<WorkspaceTaskView> { return invoke('workspaceAgent:complete', taskId, status, message) }
export function cancelWorkspaceTask(taskId: string): Promise<WorkspaceTaskView> { return invoke('workspaceAgent:cancel', taskId) }
export function resumeWorkspaceTask(taskId: string): Promise<WorkspaceTaskView> { return invoke('workspaceAgent:resume', taskId) }
export function archiveWorkspaceTask(taskId: string): Promise<WorkspaceTaskView> { return invoke('workspaceAgent:archive', taskId) }
export function restoreWorkspaceTask(taskId: string): Promise<WorkspaceTaskView> { return invoke('workspaceAgent:restore', taskId) }
export function addWorkspaceEvent(taskId: string, level: WorkspaceExecutionEvent['level'], message: string, data?: Record<string, unknown>): Promise<WorkspaceTaskView | null> { return invoke('workspaceAgent:addEvent', taskId, level, message, data && payload(data)) }
