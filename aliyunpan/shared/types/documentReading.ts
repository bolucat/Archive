export type DocumentReadingJobStatus = 'running' | 'paused' | 'completed' | 'failed' | 'cancelled' | 'stale'
export type DocumentReadingUnitStatus = 'pending' | 'running' | 'completed' | 'skipped' | 'failed'

export interface DocumentReadingUnitInput {
  index: number
  startPage: number
  endPage: number
  skipReason?: string
}

export interface DocumentReadingUnit {
  id: string
  jobId: string
  index: number
  startPage: number
  endPage: number
  status: DocumentReadingUnitStatus
  summary?: string
  keyPoints: string[]
  citationLocations: string[]
  skipReason?: string
  errorMessage?: string
  updatedAt: number
}

export interface DocumentReadingJob {
  id: string
  sourceId: string
  sourceFile: string
  totalPages: number
  status: DocumentReadingJobStatus
  summary?: string
  errorMessage?: string
  createdAt: number
  updatedAt: number
  completedAt?: number
}

export interface DocumentReadingJobView extends DocumentReadingJob {
  units: DocumentReadingUnit[]
}

export interface CreateDocumentReadingJobInput {
  sourceId: string
  sourceFile: string
  totalPages: number
  units: DocumentReadingUnitInput[]
}

/** IPC payload only: extracted text has no signed URL, token, or raw PDF bytes. */
export interface DocumentReadingPdfSection {
  index: number
  title: string
  text: string
  location: string
}

export interface DocumentReadingPdfParseResult {
  sections: DocumentReadingPdfSection[]
  totalChars: number
}
