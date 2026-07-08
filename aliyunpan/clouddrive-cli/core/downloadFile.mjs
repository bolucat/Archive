import { createWriteStream } from 'node:fs'
import { mkdir } from 'node:fs/promises'
import { dirname } from 'node:path'
import { Readable } from 'node:stream'
import { pipeline } from 'node:stream/promises'

async function responseToFile(response, outputPath) {
  if (!response?.ok) {
    const text = await response?.text?.().catch(() => '') || ''
    const err = new Error(`Download failed ${response?.status || 0}: ${text.slice(0, 200)}`)
    err.code = 'ERR_DOWNLOAD_HTTP'
    err.status = response?.status || 0
    throw err
  }
  if (!response.body) {
    const err = new Error('Download response has no body')
    err.code = 'ERR_DOWNLOAD_EMPTY_BODY'
    throw err
  }
  await mkdir(dirname(outputPath), { recursive: true })
  await pipeline(Readable.fromWeb(response.body), createWriteStream(outputPath))
}

export async function downloadResponseToFile(response, outputPath) {
  await responseToFile(response, outputPath)
}

export async function downloadUrlToFile(url, outputPath, { headers = {} } = {}) {
  const response = await fetch(url, { headers })
  await responseToFile(response, outputPath)
}
