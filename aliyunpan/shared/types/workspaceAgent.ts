export type WorkspacePlanKind = 'organize_files' | 'cleanup_duplicates' | 'cleanup_large_files' | 'cleanup_empty_directories' | 'import_share' | 'download_files'
export type WorkspaceTaskStatus = 'discovering' | 'planning' | 'awaiting_approval' | 'executing' | 'completed' | 'partial' | 'failed' | 'cancelled' | 'paused' | 'stale'
export type WorkspacePlanStatus = 'draft' | 'awaiting_approval' | 'approved' | 'executing' | 'completed' | 'partial' | 'failed' | 'rejected' | 'stale'
export type WorkspaceActionKind = 'move' | 'trash' | 'import_share' | 'download'

export interface WorkspaceDriveScope {
  userId: string
  driveId: string
  platform: string
  rootId: string
  name: string
}

export interface WorkspaceTask {
  id: string
  goal: string
  kind: WorkspacePlanKind
  scope: WorkspaceDriveScope
  status: WorkspaceTaskStatus
  summary?: string
  errorMessage?: string
  createdAt: number
  updatedAt: number
  finishedAt?: number
  archivedAt?: number
}

export interface WorkspaceEvidence {
  id: string
  taskId: string
  source: string
  summary: string
  data: Record<string, unknown>
  createdAt: number
}

export interface WorkspaceFileSnapshot {
  userId: string
  driveId: string
  fileId: string
  parentFileId?: string
  name: string
  size?: number
  time?: number
  path?: string
}

export interface WorkspacePlanAction {
  id: string
  kind: WorkspaceActionKind
  label: string
  snapshot?: WorkspaceFileSnapshot
  targetParentFileId?: string
  /** Secrets are deliberately excluded from the durable plan ledger. */
  share?: { url: string; shareId?: string; fileIds?: string[] }
}

export interface WorkspacePlan {
  id: string
  taskId: string
  kind: WorkspacePlanKind
  status: WorkspacePlanStatus
  title: string
  summary: string
  risk: string
  evidenceIds: string[]
  actions: WorkspacePlanAction[]
  hash: string
  createdAt: number
  approvedAt?: number
  completedAt?: number
}

export interface WorkspaceApproval {
  id: string
  planId: string
  planHash: string
  status: 'approved' | 'rejected'
  createdAt: number
}

export interface WorkspaceExecutionEvent {
  id: string
  taskId: string
  level: 'info' | 'warning' | 'error'
  message: string
  data?: Record<string, unknown>
  createdAt: number
}

export interface WorkspaceTaskView extends WorkspaceTask {
  evidence: WorkspaceEvidence[]
  plan?: WorkspacePlan
  approval?: WorkspaceApproval
  events: WorkspaceExecutionEvent[]
}

export interface CreateWorkspaceTaskInput {
  goal: string
  kind: WorkspacePlanKind
  scope: WorkspaceDriveScope
}

export interface CreateWorkspacePlanInput {
  taskId: string
  title: string
  summary: string
  risk: string
  evidenceIds: string[]
  actions: WorkspacePlanAction[]
}

export interface UpdateWorkspacePlanSelectionInput {
  taskId: string
  planHash: string
  actionIds: string[]
}
