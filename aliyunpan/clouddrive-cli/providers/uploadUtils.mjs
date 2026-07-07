import { createHash, createHmac } from 'node:crypto'
import { open } from 'node:fs/promises'

export async function readSlice(fileHandle, start, size) {
  const buff = Buffer.alloc(size)
  const read = await fileHandle.read(buff, 0, size, start)
  return buff.subarray(0, read.bytesRead)
}

export async function readFileBuffer(localPath) {
  const handle = await open(localPath, 'r')
  try {
    const stat = await handle.stat()
    return readSlice(handle, 0, stat.size)
  } finally {
    await handle.close().catch(() => {})
  }
}

export async function hashFile(localPath, algorithm, { chunkSize = 1024 * 1024, firstSliceSize = 0 } = {}) {
  const handle = await open(localPath, 'r')
  const full = createHash(algorithm)
  let firstSlice = ''
  try {
    const stat = await handle.stat()
    let offset = 0
    while (offset < stat.size) {
      const size = Math.min(chunkSize, stat.size - offset)
      const buff = await readSlice(handle, offset, size)
      if (!buff.length) break
      if (firstSliceSize > 0 && !firstSlice) {
        firstSlice = createHash(algorithm).update(buff.subarray(0, Math.min(firstSliceSize, buff.length))).digest('hex')
      }
      full.update(buff)
      offset += buff.length
    }
    if (stat.size === 0) firstSlice = createHash(algorithm).update(Buffer.alloc(0)).digest('hex')
    return { hash: full.digest('hex'), firstSlice, size: stat.size }
  } finally {
    await handle.close().catch(() => {})
  }
}

export async function hashBlocks(localPath, algorithm, blockSize) {
  const handle = await open(localPath, 'r')
  const full = createHash(algorithm)
  const blocks = []
  let firstSlice = ''
  try {
    const stat = await handle.stat()
    if (stat.size === 0) {
      const empty = createHash(algorithm).update(Buffer.alloc(0)).digest('hex')
      return { blocks: [empty], hash: empty, firstSlice: empty, size: 0 }
    }
    let offset = 0
    while (offset < stat.size) {
      const size = Math.min(blockSize, stat.size - offset)
      const buff = await readSlice(handle, offset, size)
      if (!firstSlice) firstSlice = createHash(algorithm).update(buff.subarray(0, Math.min(256 * 1024, buff.length))).digest('hex')
      blocks.push(createHash(algorithm).update(buff).digest('hex'))
      full.update(buff)
      offset += buff.length
    }
    return { blocks, hash: full.digest('hex'), firstSlice, size: stat.size }
  } finally {
    await handle.close().catch(() => {})
  }
}

export function buildMultipart(fields, fileField) {
  const boundary = `----clouddrive${Date.now().toString(16)}${Math.random().toString(16).slice(2)}`
  const chunks = []
  for (const [name, value] of Object.entries(fields || {})) {
    if (value === undefined || value === null) continue
    chunks.push(Buffer.from(`--${boundary}\r\n`))
    chunks.push(Buffer.from(`Content-Disposition: form-data; name="${name}"\r\n\r\n`))
    chunks.push(Buffer.from(`${value}\r\n`))
  }
  if (fileField) {
    chunks.push(Buffer.from(`--${boundary}\r\n`))
    chunks.push(Buffer.from(`Content-Disposition: form-data; name="${fileField.name}"; filename="${fileField.filename || 'file'}"\r\n`))
    chunks.push(Buffer.from('Content-Type: application/octet-stream\r\n\r\n'))
    chunks.push(fileField.body)
    chunks.push(Buffer.from('\r\n'))
  }
  chunks.push(Buffer.from(`--${boundary}--\r\n`))
  return { body: Buffer.concat(chunks), boundary }
}

export function toConflictMode(conflict) {
  if (conflict === 'overwrite') return 'overwrite'
  if (conflict === 'rename' || conflict === 'auto_rename') return 'auto_rename'
  if (conflict === 'skip' || conflict === 'refuse') return 'refuse'
  return conflict || 'refuse'
}

export function hmacSha1Base64(key, text) {
  return createHmac('sha1', key).update(text).digest('base64')
}
