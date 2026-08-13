import Database from 'better-sqlite3'
import { createHash, randomUUID } from 'crypto'
import type { CreateWorkspacePlanInput, CreateWorkspaceTaskInput, UpdateWorkspacePlanSelectionInput, WorkspaceApproval, WorkspaceEvidence, WorkspaceExecutionEvent, WorkspacePlan, WorkspacePlanAction, WorkspacePlanStatus, WorkspaceTask, WorkspaceTaskStatus, WorkspaceTaskView } from '@shared/types/workspaceAgent'

type Row = Record<string, any>
const ACTIVE = new Set<WorkspaceTaskStatus>(['discovering', 'planning', 'awaiting_approval', 'executing'])

export class WorkspaceAgentDb {
  private db: Database.Database

  constructor(path: string) {
    this.db = new Database(path)
    this.db.pragma('journal_mode = WAL')
    this.migrate()
    this.pauseInFlight()
  }

  close(): void { this.db.close() }

  createTask(input: CreateWorkspaceTaskInput, now = Date.now()): WorkspaceTaskView {
    const id = randomUUID()
    this.db.prepare('INSERT INTO workspace_tasks (id, goal, kind, scope_json, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)').run(id, input.goal.trim(), input.kind, JSON.stringify(input.scope), 'discovering', now, now)
    this.addEvent(id, 'info', '已创建工作台任务，等待取证。', undefined, now)
    return this.getTask(id)!
  }

  listTasks(limit = 50, includeArchived = false): WorkspaceTaskView[] {
    const where = includeArchived ? '' : 'WHERE archived_at IS NULL'
    return (this.db.prepare(`SELECT id FROM workspace_tasks ${where} ORDER BY updated_at DESC LIMIT ?`).all(limit) as Row[]).flatMap(row => this.getTask(row.id) || [])
  }

  getTask(id: string): WorkspaceTaskView | null {
    const row = this.db.prepare('SELECT * FROM workspace_tasks WHERE id = ?').get(id) as Row | undefined
    if (!row) return null
    const task = rowToTask(row)
    const evidence = (this.db.prepare('SELECT * FROM workspace_evidence WHERE task_id = ? ORDER BY created_at').all(id) as Row[]).map(rowToEvidence)
    const planRow = this.db.prepare('SELECT * FROM workspace_plans WHERE task_id = ? ORDER BY created_at DESC LIMIT 1').get(id) as Row | undefined
    const approvalRow = planRow ? this.db.prepare('SELECT * FROM workspace_approvals WHERE plan_id = ? ORDER BY created_at DESC LIMIT 1').get(planRow.id) as Row | undefined : undefined
    const events = (this.db.prepare('SELECT * FROM workspace_events WHERE task_id = ? ORDER BY created_at').all(id) as Row[]).map(rowToEvent)
    return { ...task, evidence, plan: planRow ? rowToPlan(planRow) : undefined, approval: approvalRow ? rowToApproval(approvalRow) : undefined, events }
  }

  addEvidence(taskId: string, source: string, summary: string, data: Record<string, unknown>, now = Date.now()): WorkspaceEvidence {
    const task = this.getTask(taskId)
    if (!task) throw new Error('工作台任务不存在')
    if (task.status === 'cancelled') throw new Error('取证已取消，结果不会写入计划')
    const id = randomUUID()
    this.db.prepare('INSERT INTO workspace_evidence (id, task_id, source, summary, data_json, created_at) VALUES (?, ?, ?, ?, ?, ?)').run(id, taskId, source, summary, JSON.stringify(data), now)
    this.transition(taskId, 'planning', undefined, now)
    return { id, taskId, source, summary, data, createdAt: now }
  }

  savePlan(input: CreateWorkspacePlanInput, now = Date.now()): WorkspacePlan {
    const task = this.getTask(input.taskId)
    if (!task) throw new Error('工作台任务不存在')
    if (task.status === 'cancelled') throw new Error('取证已取消，不能生成计划')
    if (!input.actions.length) throw new Error('没有可审批的操作')
    const id = randomUUID()
    const canonical = JSON.stringify({ taskId: input.taskId, kind: task.kind, evidenceIds: input.evidenceIds, actions: input.actions })
    const hash = createHash('sha256').update(canonical).digest('hex')
    this.db.prepare('INSERT INTO workspace_plans (id, task_id, kind, status, title, summary, risk, evidence_ids_json, actions_json, hash, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)').run(id, input.taskId, task.kind, 'awaiting_approval', input.title, input.summary, input.risk, JSON.stringify(input.evidenceIds), JSON.stringify(input.actions), hash, now)
    this.transition(input.taskId, 'awaiting_approval', input.summary, now)
    this.addEvent(input.taskId, 'warning', `计划已生成：${input.actions.length} 项操作等待整份审批。`, { planId: id, hash }, now)
    return this.getTask(input.taskId)!.plan!
  }

  approvePlan(taskId: string, planHash: string, now = Date.now()): WorkspaceTaskView {
    const view = this.getTask(taskId)
    if (!view?.plan || view.status !== 'awaiting_approval') throw new Error('当前任务没有待审批计划')
    if (view.plan.hash !== planHash) throw new Error('计划内容已变化，请重新查看后审批')
    this.db.prepare('UPDATE workspace_plans SET status = ?, approved_at = ? WHERE id = ?').run('approved', now, view.plan.id)
    this.db.prepare('INSERT INTO workspace_approvals (id, plan_id, plan_hash, status, created_at) VALUES (?, ?, ?, ?, ?)').run(randomUUID(), view.plan.id, planHash, 'approved', now)
    this.transition(taskId, 'executing', '计划已批准，正在逐项执行。', now)
    this.addEvent(taskId, 'info', '用户已批准整份计划；开始执行前将逐项复核。', { planId: view.plan.id }, now)
    return this.getTask(taskId)!
  }

  updatePlanSelection(input: UpdateWorkspacePlanSelectionInput, now = Date.now()): WorkspaceTaskView {
    const view = this.getTask(input.taskId)
    if (!view?.plan || view.status !== 'awaiting_approval') throw new Error('当前任务没有待审批计划')
    if (view.plan.hash !== input.planHash) throw new Error('计划内容已变化，请重新查看后选择')
    const selected = new Set(input.actionIds)
    const actions = view.plan.actions.filter(action => selected.has(action.id))
    if (!actions.length) throw new Error('请至少选择一个文件')
    if (actions.length !== selected.size) throw new Error('选择中包含无效项目，请重新查看计划')
    const canonical = JSON.stringify({ taskId: view.plan.taskId, kind: view.plan.kind, evidenceIds: view.plan.evidenceIds, actions })
    const hash = createHash('sha256').update(canonical).digest('hex')
    this.db.prepare('UPDATE workspace_plans SET actions_json = ?, hash = ? WHERE id = ?').run(JSON.stringify(actions), hash, view.plan.id)
    this.transition(input.taskId, 'awaiting_approval', `已选择 ${actions.length}/${view.plan.actions.length} 项，等待确认执行。`, now)
    this.addEvent(input.taskId, 'info', `用户选择执行 ${actions.length}/${view.plan.actions.length} 项；未选项目不会执行。`, { planId: view.plan.id, actionIds: actions.map(action => action.id) }, now)
    return this.getTask(input.taskId)!
  }

  rejectPlan(taskId: string, planHash: string, now = Date.now()): WorkspaceTaskView {
    const view = this.getTask(taskId)
    if (!view?.plan || view.plan.hash !== planHash) throw new Error('计划内容已变化')
    this.db.prepare('UPDATE workspace_plans SET status = ? WHERE id = ?').run('rejected', view.plan.id)
    this.db.prepare('INSERT INTO workspace_approvals (id, plan_id, plan_hash, status, created_at) VALUES (?, ?, ?, ?, ?)').run(randomUUID(), view.plan.id, planHash, 'rejected', now)
    this.transition(taskId, 'cancelled', '计划已被拒绝。', now)
    this.addEvent(taskId, 'info', '用户拒绝了整份计划，未执行任何网盘操作。', undefined, now)
    return this.getTask(taskId)!
  }

  complete(taskId: string, status: Extract<WorkspaceTaskStatus, 'completed' | 'partial' | 'failed' | 'stale'>, message: string, now = Date.now()): WorkspaceTaskView {
    const view = this.getTask(taskId)
    if (!view) throw new Error('工作台任务不存在')
    if (view.status === 'cancelled') return view
    const planStatus: WorkspacePlanStatus = status === 'completed' ? 'completed' : status
    if (view.plan) this.db.prepare('UPDATE workspace_plans SET status = ?, completed_at = ? WHERE id = ?').run(planStatus, now, view.plan.id)
    this.transition(taskId, status, message, now)
    this.addEvent(taskId, status === 'completed' ? 'info' : 'error', message, undefined, now)
    return this.getTask(taskId)!
  }

  resume(taskId: string, now = Date.now()): WorkspaceTaskView {
    const view = this.getTask(taskId)
    if (!view || view.status !== 'paused') throw new Error('只有暂停任务可以恢复')
    if (view.plan) this.db.prepare('UPDATE workspace_plans SET status = ? WHERE id = ?').run('stale', view.plan.id)
    this.transition(taskId, 'planning', '任务已恢复；正在重新取证，旧计划不可直接执行。', now)
    this.addEvent(taskId, 'warning', '已手动恢复。为避免陈旧操作，原计划已失效并需要重新取证。', undefined, now)
    return this.getTask(taskId)!
  }

  cancel(taskId: string, now = Date.now()): WorkspaceTaskView {
    const view = this.getTask(taskId)
    if (!view) throw new Error('工作台任务不存在')
    if (!['discovering', 'planning'].includes(view.status)) throw new Error('只有正在取证或规划的任务可以取消')
    this.transition(taskId, 'cancelled', '用户已取消取证，未生成或执行计划。', now)
    this.addEvent(taskId, 'info', '用户已取消取证；后续返回的扫描结果将被丢弃。', undefined, now)
    return this.getTask(taskId)!
  }

  archiveTask(taskId: string, now = Date.now()): WorkspaceTaskView {
    const view = this.getTask(taskId)
    if (!view) throw new Error('工作台任务不存在')
    if (ACTIVE.has(view.status)) throw new Error('运行中的任务不能从历史隐藏')
    this.db.prepare('UPDATE workspace_tasks SET archived_at = ? WHERE id = ?').run(now, taskId)
    this.addEvent(taskId, 'info', '任务已从默认历史列表隐藏；审计记录仍然保留。', undefined, now)
    return this.getTask(taskId)!
  }

  restoreTask(taskId: string, now = Date.now()): WorkspaceTaskView {
    const view = this.getTask(taskId)
    if (!view) throw new Error('工作台任务不存在')
    this.db.prepare('UPDATE workspace_tasks SET archived_at = NULL WHERE id = ?').run(taskId)
    this.addEvent(taskId, 'info', '任务已恢复到默认历史列表。', undefined, now)
    return this.getTask(taskId)!
  }

  addEvent(taskId: string, level: WorkspaceExecutionEvent['level'], message: string, data?: Record<string, unknown>, now = Date.now()): WorkspaceExecutionEvent {
    const id = randomUUID()
    this.db.prepare('INSERT INTO workspace_events (id, task_id, level, message, data_json, created_at) VALUES (?, ?, ?, ?, ?, ?)').run(id, taskId, level, message, data ? JSON.stringify(data) : null, now)
    return { id, taskId, level, message, data, createdAt: now }
  }

  private transition(id: string, status: WorkspaceTaskStatus, summary?: string, now = Date.now()): void {
    this.db.prepare('UPDATE workspace_tasks SET status = ?, summary = COALESCE(?, summary), updated_at = ?, finished_at = ? WHERE id = ?').run(status, summary || null, now, ['completed', 'partial', 'failed', 'cancelled', 'stale'].includes(status) ? now : null, id)
  }

  private pauseInFlight(now = Date.now()): void {
    const rows = this.db.prepare(`SELECT id FROM workspace_tasks WHERE status IN (${[...ACTIVE].map(() => '?').join(',')})`).all(...ACTIVE) as Row[]
    for (const row of rows) {
      this.db.prepare('UPDATE workspace_tasks SET status = ?, summary = ?, updated_at = ? WHERE id = ?').run('paused', '应用会话已结束，请手动恢复并重新取证。', now, row.id)
      this.addEvent(row.id, 'warning', '应用启动时发现未完成任务，已暂停等待人工恢复。', undefined, now)
    }
  }

  private migrate(): void {
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS workspace_tasks (id TEXT PRIMARY KEY, goal TEXT NOT NULL, kind TEXT NOT NULL, scope_json TEXT NOT NULL, status TEXT NOT NULL, summary TEXT, error_message TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, finished_at INTEGER);
      CREATE TABLE IF NOT EXISTS workspace_evidence (id TEXT PRIMARY KEY, task_id TEXT NOT NULL, source TEXT NOT NULL, summary TEXT NOT NULL, data_json TEXT NOT NULL, created_at INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS workspace_plans (id TEXT PRIMARY KEY, task_id TEXT NOT NULL, kind TEXT NOT NULL, status TEXT NOT NULL, title TEXT NOT NULL, summary TEXT NOT NULL, risk TEXT NOT NULL, evidence_ids_json TEXT NOT NULL, actions_json TEXT NOT NULL, hash TEXT NOT NULL, created_at INTEGER NOT NULL, approved_at INTEGER, completed_at INTEGER);
      CREATE TABLE IF NOT EXISTS workspace_approvals (id TEXT PRIMARY KEY, plan_id TEXT NOT NULL, plan_hash TEXT NOT NULL, status TEXT NOT NULL, created_at INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS workspace_events (id TEXT PRIMARY KEY, task_id TEXT NOT NULL, level TEXT NOT NULL, message TEXT NOT NULL, data_json TEXT, created_at INTEGER NOT NULL);
      CREATE INDEX IF NOT EXISTS workspace_tasks_updated ON workspace_tasks(updated_at DESC);
      CREATE INDEX IF NOT EXISTS workspace_evidence_task ON workspace_evidence(task_id, created_at);
      CREATE INDEX IF NOT EXISTS workspace_events_task ON workspace_events(task_id, created_at);
    `)
    const columns = this.db.prepare('PRAGMA table_info(workspace_tasks)').all() as Row[]
    if (!columns.some(column => column.name === 'archived_at')) this.db.exec('ALTER TABLE workspace_tasks ADD COLUMN archived_at INTEGER')
  }
}

function json<T>(value: string | null | undefined, fallback: T): T { try { return value ? JSON.parse(value) as T : fallback } catch { return fallback } }
function rowToTask(row: Row): WorkspaceTask { return { id: row.id, goal: row.goal, kind: row.kind, scope: json(row.scope_json, {}), status: row.status, summary: row.summary || undefined, errorMessage: row.error_message || undefined, createdAt: row.created_at, updatedAt: row.updated_at, finishedAt: row.finished_at || undefined, archivedAt: row.archived_at || undefined } as WorkspaceTask }
function rowToEvidence(row: Row): WorkspaceEvidence { return { id: row.id, taskId: row.task_id, source: row.source, summary: row.summary, data: json(row.data_json, {}), createdAt: row.created_at } }
function rowToPlan(row: Row): WorkspacePlan { return { id: row.id, taskId: row.task_id, kind: row.kind, status: row.status, title: row.title, summary: row.summary, risk: row.risk, evidenceIds: json(row.evidence_ids_json, []), actions: json<WorkspacePlanAction[]>(row.actions_json, []), hash: row.hash, createdAt: row.created_at, approvedAt: row.approved_at || undefined, completedAt: row.completed_at || undefined } }
function rowToApproval(row: Row): WorkspaceApproval { return { id: row.id, planId: row.plan_id, planHash: row.plan_hash, status: row.status, createdAt: row.created_at } }
function rowToEvent(row: Row): WorkspaceExecutionEvent { return { id: row.id, taskId: row.task_id, level: row.level, message: row.message, data: json(row.data_json, undefined), createdAt: row.created_at } }
