import Database from 'better-sqlite3'
import { randomUUID } from 'crypto'
import type { CreateDocumentReadingJobInput, DocumentReadingJob, DocumentReadingJobStatus, DocumentReadingJobView, DocumentReadingUnit, DocumentReadingUnitStatus } from '@shared/types/documentReading'

type Row = Record<string, any>

export class DocumentReadingDb {
  private db: Database.Database

  constructor(path: string) {
    this.db = new Database(path)
    this.db.pragma('journal_mode = WAL')
    this.migrate()
    this.pauseInterruptedJobs()
  }

  close(): void { this.db.close() }

  create(input: CreateDocumentReadingJobInput, now = Date.now()): DocumentReadingJobView {
    if (!input.sourceId || !input.sourceFile) throw new Error('阅读来源不完整')
    if (!Number.isInteger(input.totalPages) || input.totalPages < 1) throw new Error('PDF 页数无效')
    if (!input.units.length) throw new Error('阅读单元不能为空')
    const id = randomUUID()
    const insert = this.db.prepare('INSERT INTO document_reading_jobs (id, source_id, source_file, total_pages, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)')
    const insertUnit = this.db.prepare('INSERT INTO document_reading_units (id, job_id, unit_index, start_page, end_page, status, skip_reason, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)')
    this.db.transaction(() => {
      insert.run(id, input.sourceId, input.sourceFile, input.totalPages, 'paused', now, now)
      for (const unit of input.units) insertUnit.run(randomUUID(), id, unit.index, unit.startPage, unit.endPage, unit.skipReason ? 'skipped' : 'pending', unit.skipReason || null, now)
    })()
    return this.get(id)!
  }

  get(id: string): DocumentReadingJobView | null {
    const row = this.db.prepare('SELECT * FROM document_reading_jobs WHERE id = ?').get(id) as Row | undefined
    if (!row) return null
    const units = (this.db.prepare('SELECT * FROM document_reading_units WHERE job_id = ? ORDER BY unit_index').all(id) as Row[]).map(toUnit)
    return { ...toJob(row), units }
  }

  listForSource(sourceId: string): DocumentReadingJobView[] {
    return (this.db.prepare('SELECT id FROM document_reading_jobs WHERE source_id = ? ORDER BY updated_at DESC').all(sourceId) as Row[]).flatMap(row => this.get(row.id) || [])
  }

  setJobStatus(id: string, status: DocumentReadingJobStatus, errorMessage?: string, now = Date.now()): DocumentReadingJobView {
    const completedAt = ['completed', 'failed', 'cancelled', 'stale'].includes(status) ? now : null
    this.db.prepare('UPDATE document_reading_jobs SET status = ?, error_message = ?, updated_at = ?, completed_at = ? WHERE id = ?').run(status, errorMessage || null, now, completedAt, id)
    const job = this.get(id)
    if (!job) throw new Error('阅读任务不存在')
    return job
  }

  completeUnit(input: { jobId: string; index: number; summary: string; keyPoints: string[]; citationLocations: string[] }, now = Date.now()): DocumentReadingJobView {
    this.db.prepare('UPDATE document_reading_units SET status = ?, summary = ?, key_points_json = ?, citations_json = ?, error_message = NULL, updated_at = ? WHERE job_id = ? AND unit_index = ?').run('completed', input.summary, JSON.stringify(input.keyPoints), JSON.stringify(input.citationLocations), now, input.jobId, input.index)
    this.db.prepare('UPDATE document_reading_jobs SET updated_at = ? WHERE id = ?').run(now, input.jobId)
    return this.get(input.jobId)!
  }

  failUnit(jobId: string, index: number, message: string, now = Date.now()): DocumentReadingJobView {
    this.db.prepare('UPDATE document_reading_units SET status = ?, error_message = ?, updated_at = ? WHERE job_id = ? AND unit_index = ?').run('failed', message, now, jobId, index)
    this.db.prepare('UPDATE document_reading_jobs SET updated_at = ? WHERE id = ?').run(now, jobId)
    return this.get(jobId)!
  }

  private pauseInterruptedJobs(now = Date.now()): void {
    this.db.prepare("UPDATE document_reading_jobs SET status = 'paused', updated_at = ? WHERE status = 'running'").run(now)
    this.db.prepare("UPDATE document_reading_units SET status = 'pending', updated_at = ? WHERE status = 'running'").run(now)
  }

  private migrate(): void {
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS document_reading_jobs (
        id TEXT PRIMARY KEY, source_id TEXT NOT NULL, source_file TEXT NOT NULL, total_pages INTEGER NOT NULL,
        status TEXT NOT NULL, summary TEXT, error_message TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, completed_at INTEGER
      );
      CREATE TABLE IF NOT EXISTS document_reading_units (
        id TEXT PRIMARY KEY, job_id TEXT NOT NULL, unit_index INTEGER NOT NULL, start_page INTEGER NOT NULL, end_page INTEGER NOT NULL,
        status TEXT NOT NULL, summary TEXT, key_points_json TEXT, citations_json TEXT, skip_reason TEXT, error_message TEXT, updated_at INTEGER NOT NULL,
        UNIQUE(job_id, unit_index)
      );
      CREATE INDEX IF NOT EXISTS document_reading_jobs_source ON document_reading_jobs(source_id, updated_at DESC);
      CREATE INDEX IF NOT EXISTS document_reading_units_job ON document_reading_units(job_id, unit_index);
    `)
  }
}

function parse<T>(value: string | null, fallback: T): T { try { return value ? JSON.parse(value) as T : fallback } catch { return fallback } }
function toJob(row: Row): DocumentReadingJob {
  return { id: row.id, sourceId: row.source_id, sourceFile: row.source_file, totalPages: row.total_pages, status: row.status, summary: row.summary || undefined, errorMessage: row.error_message || undefined, createdAt: row.created_at, updatedAt: row.updated_at, completedAt: row.completed_at || undefined }
}
function toUnit(row: Row): DocumentReadingUnit {
  return { id: row.id, jobId: row.job_id, index: row.unit_index, startPage: row.start_page, endPage: row.end_page, status: row.status as DocumentReadingUnitStatus, summary: row.summary || undefined, keyPoints: parse(row.key_points_json, []), citationLocations: parse(row.citations_json, []), skipReason: row.skip_reason || undefined, errorMessage: row.error_message || undefined, updatedAt: row.updated_at }
}
