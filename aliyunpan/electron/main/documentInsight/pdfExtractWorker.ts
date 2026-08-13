import { parentPort, workerData } from 'worker_threads'
import { readFileSync } from 'fs'

async function run() {
  const pdfjs = await import('pdfjs-dist/build/pdf.js') as any
  const data = new Uint8Array(readFileSync(workerData.filePath))
  const document = await pdfjs.getDocument({ data, disableWorker: true }).promise
  const sections: Array<{ index: number; title: string; text: string; location: string }> = []
  let totalChars = 0
  for (let pageNumber = 1; pageNumber <= document.numPages; pageNumber++) {
    const page = await document.getPage(pageNumber)
    const content = await page.getTextContent()
    const text = content.items.map((item: any) => item.str || '').join(' ').replace(/\s+/g, ' ').trim()
    if (text) {
      totalChars += text.length
      sections.push({ index: pageNumber - 1, title: `第 ${pageNumber} 页`, text, location: `page:${pageNumber}` })
    }
    parentPort?.postMessage({ type: 'progress', current: pageNumber, total: document.numPages })
  }
  parentPort?.postMessage({ type: 'done', sections, totalChars })
}

run().catch(error => parentPort?.postMessage({ type: 'error', message: error instanceof Error ? error.message : String(error) }))
