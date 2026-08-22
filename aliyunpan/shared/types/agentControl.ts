export type AgentSurface = 'ai_search' | 'workspace' | 'media_acquisition' | 'document_reading'
export type AgentWorkflowStatus = 'gathering_evidence' | 'awaiting_approval' | 'approved' | 'executing' | 'completed' | 'failed' | 'cancelled' | 'expired'
export type AgentPlanStatus = 'awaiting_approval' | 'approved' | 'completed' | 'failed' | 'cancelled' | 'stale'
export type AgentExecutionGrantStatus = 'pending' | 'claimed' | 'completed' | 'failed' | 'expired'
export type AgentReceiptStatus = 'succeeded' | 'failed'
export type AgentActionKind = 'move' | 'trash' | 'import_share' | 'download' | 'offline_download'
export type AgentResourceKind = 'file' | 'folder' | 'share' | 'external_source'

export interface AgentScopeGrant {
  accountId: string
  driveId: string
  rootId: string
  platform: string
  operations: string[]
}

export interface AgentWorkflow {
  id: string
  surface: AgentSurface
  goal: string
  scope: AgentScopeGrant
  status: AgentWorkflowStatus
  externalRef?: string
  createdAt: number
  updatedAt: number
  finishedAt?: number
}

export interface AgentEvidence {
  id: string
  workflowId: string
  source: string
  summary: string
  data: Record<string, unknown>
  hash: string
  externalRef?: string
  createdAt: number
}

export interface AgentResourceSnapshot {
  accountId: string
  driveId: string
  resourceId: string
  parentResourceId?: string
  name: string
}

export interface AgentResourceHandle {
  id: string
  workflowId: string
  evidenceId: string
  kind: AgentResourceKind
  snapshot: AgentResourceSnapshot
  hash: string
  externalRef?: string
  createdAt: number
}

export interface AgentPlanAction {
  id: string
  kind: AgentActionKind
  label: string
  idempotencyKey: string
  resourceHandleIds: string[]
  parameters?: Record<string, unknown>
}

export interface AgentPlan {
  id: string
  workflowId: string
  status: AgentPlanStatus
  summary: string
  risk: string
  evidenceIds: string[]
  actions: AgentPlanAction[]
  hash: string
  createdAt: number
  approvedAt?: number
  completedAt?: number
}

export interface AgentApproval {
  id: string
  planId: string
  planHash: string
  status: 'approved'
  createdAt: number
}

export interface AgentExecutionGrant {
  id: string
  workflowId: string
  planId: string
  planHash: string
  status: AgentExecutionGrantStatus
  expiresAt: number
  workerId?: string
  leaseExpiresAt?: number
  createdAt: number
  completedAt?: number
}

export interface AgentActionReceipt {
  id: string
  executionGrantId: string
  actionId: string
  idempotencyKey: string
  status: AgentReceiptStatus
  result?: Record<string, unknown>
  createdAt: number
}

export interface AgentLedgerEvent {
  id: string
  workflowId: string
  level: 'info' | 'warning' | 'error'
  message: string
  data?: Record<string, unknown>
  createdAt: number
}

export interface AgentWorkflowView extends AgentWorkflow {
  evidence: AgentEvidence[]
  resourceHandles: AgentResourceHandle[]
  plan?: AgentPlan
  approval?: AgentApproval
  executionGrant?: AgentExecutionGrant
  receipts: AgentActionReceipt[]
  events: AgentLedgerEvent[]
}

export interface CreateAgentWorkflowInput {
  surface: AgentSurface
  goal: string
  scope: AgentScopeGrant
  externalRef?: string
}

export interface CreateAgentEvidenceInput {
  source: string
  summary: string
  data: Record<string, unknown>
  externalRef?: string
}

export interface CreateAgentPlanInput {
  summary: string
  risk: string
  evidenceIds: string[]
  actions: AgentPlanAction[]
}

export interface CreateAgentResourceHandleInput {
  evidenceId: string
  kind: AgentResourceKind
  snapshot: AgentResourceSnapshot
  externalRef?: string
}

export interface RecordAgentActionReceiptInput {
  actionId: string
  idempotencyKey: string
  status: AgentReceiptStatus
  result?: Record<string, unknown>
}
