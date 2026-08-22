import type { AgentPlanAction, AgentResourceSnapshot, AgentScopeGrant, CreateAgentEvidenceInput } from '@shared/types/agentControl'
import { addAgentEvidence, approveAgentPlan, completeAgentObservation, createAgentPlan, createAgentResourceHandle, createAgentWorkflow, findAgentWorkflowByExternalRef, getAgentWorkflow } from '../agent/AgentControlService'
import { CloudDriveCliAdapter } from '../agent/CloudDriveCliAdapter'
import { agentV1FeatureFlags } from '../agent/AgentV1FeatureFlags'

export interface AiSearchV1Target {
  userId: string
  driveId: string
  rootId: string
  platform: string
}

export interface BeginAiSearchV1AuditInput {
  sessionId: string
  runId: string
  goal: string
  target?: AiSearchV1Target
}

export interface RecordAiSearchV1EvidenceInput {
  sessionId: string
  runId: string
  toolCallId: string
  toolName: string
  isError: boolean
  result: unknown
}

export interface ConfirmAiSearchV1WriteInput {
  confirmationId: string
  kind: 'move' | 'trash'
  target: AiSearchV1Target
  fileIds: string[]
  targetParentFileId?: string
}

export interface AiSearchStorageStats {
  provider: string
  driveId: string
  maxDepth: number
  totalFiles: number
  totalDirs: number
  totalSize: number
  byCategory: Record<string, { count: number; size: number }>
  topExtensions: Record<string, number>
}

const workflowRef = (sessionId: string, runId: string) => `ai-search:${sessionId}:${runId}`

export function aiSearchScope(target: AiSearchV1Target): AgentScopeGrant {
  return { accountId: target.userId, driveId: target.driveId, rootId: target.rootId, platform: target.platform, operations: ['files.list'] }
}

export function aiSearchWriteScope(target: AiSearchV1Target, kind: ConfirmAiSearchV1WriteInput['kind']): AgentScopeGrant {
  return { ...aiSearchScope(target), operations: ['files.list', kind === 'move' ? 'files.move' : 'trash.move'] }
}

export function aiSearchWriteAction(input: ConfirmAiSearchV1WriteInput, snapshot: AgentResourceSnapshot, handleId: string): AgentPlanAction {
  return {
    id: `ai-search:${safeName(input.confirmationId)}:${safeName(snapshot.resourceId)}`,
    kind: input.kind,
    label: input.kind === 'move' ? `移动：${snapshot.name}` : `移入回收站：${snapshot.name}`,
    idempotencyKey: `ai-search:${safeName(input.confirmationId)}:${safeName(snapshot.resourceId)}`,
    resourceHandleIds: [handleId],
    parameters: input.kind === 'move' ? { targetParentFileId: input.targetParentFileId } : undefined
  }
}

export function aiSearchEvidence(input: RecordAiSearchV1EvidenceInput): CreateAgentEvidenceInput {
  const data = compactToolResult(input.result)
  return {
    source: `ai_search.${safeName(input.toolName)}`,
    summary: `${safeName(input.toolName)} ${input.isError ? '失败' : '已完成'}`,
    data: { toolName: safeName(input.toolName), isError: input.isError, ...data },
    externalRef: `${workflowRef(input.sessionId, input.runId)}:tool:${safeName(input.toolCallId)}`
  }
}

/** Shadow-only audit for a user-selected single drive. AI Search keeps its legacy tool runtime for now. */
export function beginAiSearchV1Audit(input: BeginAiSearchV1AuditInput): string | null {
  if (!agentV1FeatureFlags().aiSearchBridge || !input.target || !input.sessionId || !input.runId || !input.goal.trim()) return null
  const ref = workflowRef(input.sessionId, input.runId)
  const existing = findAgentWorkflowByExternalRef(ref)
  if (existing) return existing.id
  return createAgentWorkflow({ surface: 'ai_search', goal: input.goal.slice(0, 1000), scope: aiSearchScope(input.target), externalRef: ref }).id
}

export function recordAiSearchV1Evidence(input: RecordAiSearchV1EvidenceInput): void {
  if (!agentV1FeatureFlags().aiSearchBridge) return
  const workflow = findAgentWorkflowByExternalRef(workflowRef(input.sessionId, input.runId))
  if (!workflow || workflow.status !== 'gathering_evidence') return
  addAgentEvidence(workflow.id, aiSearchEvidence(input))
}

export function finishAiSearchV1Audit(sessionId: string, runId: string, status: 'completed' | 'failed' | 'cancelled' = 'completed', message = 'AI 搜索对话已结束'): void {
  if (!agentV1FeatureFlags().aiSearchBridge) return
  const workflow = findAgentWorkflowByExternalRef(workflowRef(sessionId, runId))
  if (workflow?.status === 'gathering_evidence') completeAgentObservation(workflow.id, status, message)
}

/**
 * Run a bounded, read-only directory statistic through the main-process adapter.
 * The renderer supplies only a ledger workflow id; the approved scope is read back
 * from the ledger and cannot be widened by a tool call.
 */
export async function collectAiSearchStorageStats(workflowId: string): Promise<AiSearchStorageStats> {
  if (!agentV1FeatureFlags().aiSearchBridge) throw new Error('AI 搜索 Agent V1 未启用')
  const workflow = getAgentWorkflow(workflowId)
  if (!workflow || workflow.surface !== 'ai_search' || workflow.status !== 'gathering_evidence') throw new Error('只读统计工作流不存在或已结束')
  const stats = await new CloudDriveCliAdapter().getDirectoryStats(workflow.scope, 2)
  const byCategory = Object.fromEntries(Object.entries(stats.by_category).map(([name, value]) => [name, { count: Number(value.count || 0), size: Number(value.size || 0) }]))
  const result: AiSearchStorageStats = {
    provider: stats.provider,
    driveId: stats.driveId,
    maxDepth: stats.max_depth,
    totalFiles: stats.total_files,
    totalDirs: stats.total_dirs,
    totalSize: stats.total_size,
    byCategory,
    topExtensions: stats.top_extensions
  }
  addAgentEvidence(workflow.id, {
    source: 'ai_search.storage_stats',
    summary: `只读统计完成：${result.totalFiles} 个文件、${result.totalDirs} 个目录`,
    data: { maxDepth: result.maxDepth, totalFiles: result.totalFiles, totalDirs: result.totalDirs, totalSize: result.totalSize, byCategory: result.byCategory, topExtensions: result.topExtensions },
    externalRef: `${workflow.externalRef || workflow.id}:storage-stats`
  })
  return result
}

/**
 * The confirmation card is the user approval. The renderer supplies IDs only;
 * the main process re-inspects every resource before recording the frozen plan.
 */
export async function confirmAiSearchV1Write(input: ConfirmAiSearchV1WriteInput): Promise<{ workflowId: string } | null> {
  if (!agentV1FeatureFlags().aiSearchCutover) return null
  validateWriteInput(input)
  const ref = `ai-search-write:${safeName(input.confirmationId)}`
  const existing = findAgentWorkflowByExternalRef(ref)
  if (existing) return { workflowId: existing.id }

  const scope = aiSearchWriteScope(input.target, input.kind)
  const inspector = new CloudDriveCliAdapter()
  const workflow = createAgentWorkflow({ surface: 'ai_search', goal: input.kind === 'move' ? '按已确认的 AI 搜索结果移动文件' : '按已确认的 AI 搜索结果移入回收站', scope, externalRef: ref })
  const evidenceView = addAgentEvidence(workflow.id, {
    source: 'ai_search.confirmation_card',
    summary: input.kind === 'move' ? `用户确认移动 ${input.fileIds.length} 个文件` : `用户确认移入回收站 ${input.fileIds.length} 个文件`,
    data: { kind: input.kind, fileCount: input.fileIds.length, targetParentFileId: input.kind === 'move' ? input.targetParentFileId : undefined },
    externalRef: `${ref}:confirmation`
  })
  const evidence = evidenceView?.evidence.find(item => item.externalRef === `${ref}:confirmation`)
  if (!evidence) throw new Error('无法记录 AI 搜索确认凭证')

  if (input.kind === 'move') {
    const target = await inspector.inspectResource(scope, input.targetParentFileId!)
    if (target.kind !== 'folder' || !await inspector.isWithinScopeRoot(scope, input.targetParentFileId!)) throw new Error('移动目标不在授权范围内或不是目录')
  }

  const actions: AgentPlanAction[] = []
  for (const fileId of input.fileIds) {
    if (!await inspector.isWithinScopeRoot(scope, fileId)) throw new Error('文件已不在授权根目录内，请重新搜索后再确认')
    const current = await inspector.inspectResource(scope, fileId)
    const handle = createAgentResourceHandle(workflow.id, { evidenceId: evidence.id, kind: current.kind, snapshot: current.snapshot, externalRef: `${ref}:resource:${safeName(fileId)}` })
    actions.push(aiSearchWriteAction(input, current.snapshot, handle.id))
  }
  const planned = createAgentPlan(workflow.id, {
    summary: input.kind === 'move' ? `移动 ${actions.length} 个已确认文件` : `将 ${actions.length} 个已确认文件移入回收站`,
    risk: input.kind === 'move' ? '文件路径会改变；执行前会重新校验文件与目标目录。' : '文件会移入网盘回收站；执行前会重新校验文件位置。',
    evidenceIds: [evidence.id],
    actions
  })
  if (!planned?.plan) throw new Error('无法生成 AI 搜索执行计划')
  approveAgentPlan(workflow.id, planned.plan.hash)
  return { workflowId: workflow.id }
}

function validateWriteInput(input: ConfirmAiSearchV1WriteInput): void {
  if (!input.confirmationId || !input.target?.userId || !input.target.driveId || !input.target.rootId || !input.target.platform) throw new Error('AI 搜索确认缺少网盘授权范围')
  if (!Array.isArray(input.fileIds) || !input.fileIds.length || input.fileIds.length > 500) throw new Error('请选择 1 到 500 个文件后再确认')
  if (new Set(input.fileIds).size !== input.fileIds.length || input.fileIds.some(id => !String(id || '').trim())) throw new Error('确认文件列表无效')
  if (input.kind === 'move' && !String(input.targetParentFileId || '').trim()) throw new Error('移动操作缺少目标目录')
}

function compactToolResult(result: unknown): Record<string, unknown> {
  const value = result && typeof result === 'object' && (result as any).details !== undefined ? (result as any).details : result
  if (!value || typeof value !== 'object' || Array.isArray(value)) return { resultType: Array.isArray(value) ? 'array' : typeof value }
  const source = value as Record<string, unknown>
  const output: Record<string, unknown> = {}
  for (const key of ['total', 'count', 'success', 'failed', 'savedCount', 'fileCount', 'scannedDirs', 'awaitingSelection']) {
    const item = source[key]
    if (typeof item === 'number' || typeof item === 'boolean') output[key] = item
  }
  if (typeof source.error === 'string') output.error = redactText(source.error)
  return Object.keys(output).length ? output : { resultType: 'object' }
}

function safeName(value: string): string {
  return String(value || 'unknown').replace(/[^a-zA-Z0-9_.:-]/g, '_').slice(0, 120) || 'unknown'
}

function redactText(value: string): string {
  return value.replace(/https?:\/\/\S+/gi, '[链接已脱敏]').replace(/(?:password|pwd|密码|token|secret)\s*[:：=]\s*\S+/gi, '[敏感信息已脱敏]').slice(0, 300)
}
