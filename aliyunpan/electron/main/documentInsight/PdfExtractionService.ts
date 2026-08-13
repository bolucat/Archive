import { app, net } from 'electron'
import { createWriteStream, mkdirSync, rmSync } from 'fs'
import { join } from 'path'
import { Worker } from 'worker_threads'
import { randomUUID } from 'crypto'
import type { DocumentReadingPdfParseResult } from '@shared/types/documentReading'

const MAX_BYTES = 200 * 1024 * 1024

function temporaryPath(): { directory: string; filePath: string } {
  const directory = join(app.getPath('temp'), 'boxplayer-document-ai', randomUUID())
  mkdirSync(directory, { recursive: true })
  return { directory, filePath: join(directory, `${randomUUID()}.pdf`) }
}

async function downloadPdf(url: string, headers: Record<string, string>, filePath: string, signal?: AbortSignal, onProgress?: (loaded: number, total?: number) => void): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const request = net.request({ method: 'GET', url, useSessionCookies: true })
    for (const [name, value] of Object.entries(headers || {})) request.setHeader(name, value)
    const abort = () => { request.abort(); reject(new DOMException('Aborted', 'AbortError')) }
    signal?.addEventListener('abort', abort, { once: true })
    request.on('response', response => {
      if (response.statusCode < 200 || response.statusCode >= 300) { request.abort(); reject(new Error(`下载失败: HTTP ${response.statusCode}`)); return }
      const total = Number(response.headers['content-length'] || 0) || undefined
      let loaded = 0
      const output = createWriteStream(filePath, { mode: 0o600 })
      response.on('data', chunk => {
        loaded += chunk.length
        if (loaded > MAX_BYTES) { request.abort(); output.destroy(); reject(new Error('超过 200 MB 限制')); return }
        onProgress?.(loaded, total)
      })
      response.on('error', reject)
      output.on('error', reject)
      output.on('finish', resolve)
      response.pipe(output)
    })
    request.on('error', reject)
    request.end()
  })
}

function extractWithWorker(filePath: string, signal?: AbortSignal, onProgress?: (current: number, total: number) => void): Promise<DocumentReadingPdfParseResult> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(join(__dirname, 'pdfExtractWorker.js'), { workerData: { filePath } })
    const abort = () => { void worker.terminate(); reject(new DOMException('Aborted', 'AbortError')) }
    signal?.addEventListener('abort', abort, { once: true })
    worker.on('message', message => {
      if (message?.type === 'progress') onProgress?.(message.current, message.total)
      if (message?.type === 'done') resolve({ sections: message.sections || [], totalChars: message.totalChars || 0 })
      if (message?.type === 'error') reject(new Error(message.message || 'PDF 解析失败'))
    })
    worker.once('error', reject)
    worker.once('exit', code => { if (code !== 0) reject(new Error(`PDF 解析进程异常退出 (${code})`)) })
  })
}

/** Downloads into the OS temp area and removes the encrypted-mode temp file after parsing. */
export async function downloadAndExtractPdf(input: { url: string; headers?: Record<string, string>; signal?: AbortSignal; onProgress?: (event: { phase: 'download' | 'parsing'; current: number; total?: number }) => void }): Promise<DocumentReadingPdfParseResult> {
  if (!input.url.startsWith('https://') && !input.url.startsWith('http://')) throw new Error('PDF 下载地址无效')
  const { directory, filePath } = temporaryPath()
  try {
    await downloadPdf(input.url, input.headers || {}, filePath, input.signal, (current, total) => input.onProgress?.({ phase: 'download', current, total }))
    return await extractWithWorker(filePath, input.signal, (current, total) => input.onProgress?.({ phase: 'parsing', current, total }))
  } finally {
    rmSync(filePath, { force: true })
    try { rmSync(directory, { recursive: true, force: true }) } catch {}
  }
}
