import { app } from 'electron'
import { join } from 'path'
import type { CreateDocumentReadingJobInput, DocumentReadingJobStatus, DocumentReadingJobView } from '@shared/types/documentReading'
import { DocumentReadingDb } from './DocumentReadingDb'

let db: DocumentReadingDb | null = null

function getDb(): DocumentReadingDb {
  if (!db) db = new DocumentReadingDb(join(app.getPath('userData'), 'document-reading.db'))
  return db
}

export function createDocumentReadingJob(input: CreateDocumentReadingJobInput): DocumentReadingJobView { return getDb().create(input) }
export function getDocumentReadingJob(id: string): DocumentReadingJobView | null { return getDb().get(id) }
export function listDocumentReadingJobs(sourceId: string): DocumentReadingJobView[] { return getDb().listForSource(sourceId) }
export function setDocumentReadingJobStatus(id: string, status: DocumentReadingJobStatus, errorMessage?: string): DocumentReadingJobView { return getDb().setJobStatus(id, status, errorMessage) }
export function completeDocumentReadingUnit(input: { jobId: string; index: number; summary: string; keyPoints: string[]; citationLocations: string[] }): DocumentReadingJobView { return getDb().completeUnit(input) }
export function failDocumentReadingUnit(jobId: string, index: number, message: string): DocumentReadingJobView { return getDb().failUnit(jobId, index, message) }
export function destroyDocumentReadingDb(): void { db?.close(); db = null }
