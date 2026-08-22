import type { AgentScopeGrant } from '@shared/types/agentControl'
import { addAgentEvidence, completeAgentObservation, createAgentWorkflow, findAgentWorkflowByExternalRef, getAgentWorkflow } from '../agent/AgentControlService'
import { agentV1FeatureFlags } from '../agent/AgentV1FeatureFlags'

export interface DocumentAgentV1Source {
  sourceId: string
  fileName: string
  userId: string
  driveId: string
  fileId: string
  parentFileId?: string
  platform: string
}

const safeRef = (value: string) => String(value || 'unknown').replace(/[^a-zA-Z0-9_.:-]/g, '_').slice(0, 160) || 'unknown'
const workflowRef = (sessionId: string, runId: string) => `document:${safeRef(sessionId)}:${safeRef(runId)}`

function documentScope(source: DocumentAgentV1Source): AgentScopeGrant {
  return { accountId: source.userId, driveId: source.driveId, rootId: source.parentFileId || source.fileId, platform: source.platform, operations: ['files.list', 'files.download'] }
}

/** Records document AI activity without copying document text, download URLs, or model prompts into the ledger. */
export function beginDocumentV1Audit(input: { sessionId: string; runId: string; goal: string; sources: DocumentAgentV1Source[] }): string | null {
  if (!agentV1FeatureFlags().documentBridge || !input.goal.trim() || !input.sessionId || !input.runId || !input.sources.length) return null
  const first = input.sources[0]
  if (!first.userId || !first.driveId || !first.fileId || !first.platform || input.sources.some(source => source.userId !== first.userId || source.driveId !== first.driveId || !source.fileId || !source.platform)) return null
  const ref = workflowRef(input.sessionId, input.runId)
  const existing = findAgentWorkflowByExternalRef(ref)
  if (existing) return existing.id
  const workflow = createAgentWorkflow({ surface: 'document_reading', goal: input.goal.slice(0, 1000), scope: documentScope(first), externalRef: ref })
  for (const source of input.sources) {
    addAgentEvidence(workflow.id, {
      source: 'document_source',
      summary: `已选择文档：${source.fileName || '未命名文档'}`,
      data: { sourceId: source.sourceId, fileId: source.fileId, fileName: source.fileName },
      externalRef: `${ref}:source:${safeRef(source.sourceId)}`
    })
  }
  return workflow.id
}

export function recordDocumentV1Citation(workflowId: string, input: { sourceId: string; location: string }): void {
  if (!agentV1FeatureFlags().documentBridge) return
  const workflow = findWorkflow(workflowId)
  if (!workflow || workflow.status !== 'gathering_evidence') return
  addAgentEvidence(workflow.id, {
    source: 'document_citation',
    summary: `已检索文档位置：${input.location || '正文'}`,
    data: { sourceId: input.sourceId, location: input.location || '正文' },
    externalRef: `document-citation:${safeRef(input.sourceId)}:${safeRef(input.location || 'body')}`
  })
}

export function finishDocumentV1Audit(workflowId: string, status: 'completed' | 'failed' | 'cancelled', message: string): void {
  if (!agentV1FeatureFlags().documentBridge) return
  const workflow = findWorkflow(workflowId)
  if (workflow?.status === 'gathering_evidence') completeAgentObservation(workflow.id, status, message)
}

function findWorkflow(workflowId: string) {
  try { return workflowId ? getAgentWorkflow(workflowId) : null } catch { return null }
}
