import type { CreateDocumentReadingJobInput, DocumentReadingJobStatus, DocumentReadingJobView, DocumentReadingPdfParseResult } from '@shared/types/documentReading'

function invoke<T>(channel: string, ...args: unknown[]): Promise<T> {
  const ipc = window.Electron?.ipcRenderer
  if (!ipc) return Promise.reject(new Error('深度阅读仅支持桌面客户端'))
  return ipc.invoke(channel, ...args) as Promise<T>
}

export function createDocumentReadingJob(input: CreateDocumentReadingJobInput): Promise<DocumentReadingJobView> { return invoke('documentReading:create', input) }
export function getDocumentReadingJob(id: string): Promise<DocumentReadingJobView | null> { return invoke('documentReading:get', id) }
export function listDocumentReadingJobs(sourceId: string): Promise<DocumentReadingJobView[]> { return invoke('documentReading:listForSource', sourceId) }
export function setDocumentReadingJobStatus(id: string, status: DocumentReadingJobStatus, errorMessage?: string): Promise<DocumentReadingJobView> { return invoke('documentReading:setStatus', id, status, errorMessage) }
export function completeDocumentReadingUnit(input: { jobId: string; index: number; summary: string; keyPoints: string[]; citationLocations: string[] }): Promise<DocumentReadingJobView> { return invoke('documentReading:completeUnit', input) }
export function failDocumentReadingUnit(jobId: string, index: number, message: string): Promise<DocumentReadingJobView> { return invoke('documentReading:failUnit', jobId, index, message) }
export function extractPdfForDocumentReading(input: { url: string; headers?: Record<string, string>; onProgress?: (progress: { phase: 'download' | 'parsing'; current: number; total?: number }) => void }): Promise<DocumentReadingPdfParseResult> {
  if (window.DocumentReadingExtractPdf) return window.DocumentReadingExtractPdf(input, input.onProgress)
  return invoke('documentReading:extractPdf', { url: input.url, headers: input.headers })
}
