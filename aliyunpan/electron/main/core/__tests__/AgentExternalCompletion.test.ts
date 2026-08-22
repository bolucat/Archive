import { describe, expect, it } from 'vitest'
import { AgentLedgerDb } from '../../agent/AgentLedgerDb'

describe('AgentLedgerDb external completion', () => {
  it('closes a shadow workflow when its legacy runner fails before planning', () => {
    const db = new AgentLedgerDb(':memory:')
    const workflow = db.createWorkflow({ surface: 'workspace', goal: 'test', scope: { accountId: 'user-1', driveId: 'drive-1', rootId: 'root', platform: 'aliyun', operations: ['files.list'] } })
    const completed = db.completeExternalWorkflow(workflow.id, 'failed', 'legacy lookup failed')
    expect(completed.status).toBe('failed')
    expect(completed.events.at(-1)?.message).toBe('legacy lookup failed')
  })
})
