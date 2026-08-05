import crypto from 'crypto'
import path from 'path'
import { OpenFileHandle } from '../utils/filehelper'
import { IUploadingUI } from '../utils/dbupload'
import AliUploadDisk from '../aliapi/uploaddisk'
import { Sleep } from '../utils/format'
import { quarkRequest } from './dirfilelist'

const DEFAULT_PART_SIZE = 10 * 1024 * 1024
const OSS_USER_AGENT = 'aliyun-sdk-js/1.0.0 Chrome 145.0.0.0 on Windows 10 64-bit'
const PART_TIMEOUT = 30 * 60 * 1000
const COMMIT_TIMEOUT = 5 * 60 * 1000

type QuarkUploadTask = {
  task_id?: string
  auth_info?: unknown
  upload_id?: string
  obj_key?: string
  bucket?: string
  upload_url?: string
  callback?: unknown
  part_size?: number
}

type HashContext = {
  hash_type: 'sha1'
  h0: string
  h1: string
  h2: string
  h3: string
  h4: string
  Nl: string
  Nh: string
  data: string
  num: string
}

const getMimeType = (name: string) => {
  const ext = name.slice(name.lastIndexOf('.')).toLowerCase()
  return ({ '.txt': 'text/plain', '.json': 'application/json', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.png': 'image/png', '.gif': 'image/gif', '.mp3': 'audio/mpeg', '.mp4': 'video/mp4', '.pdf': 'application/pdf', '.zip': 'application/zip' } as Record<string, string>)[ext] || 'application/octet-stream'
}

const isError = (value: any): value is { __error: true; message: string } => !!value?.__error

const requestData = async (userId: string, endpoint: string, body: Record<string, unknown>) => {
  const response = await quarkRequest<any>(userId, endpoint, { method: 'POST', body: JSON.stringify(body) })
  if (!response || isError(response)) throw new Error(response?.message || '夸克网盘上传请求失败')
  const data = response.data || {}
  if (response.metadata?.part_size && !data.part_size) data.part_size = response.metadata.part_size
  return data
}

const toOssUrl = (task: QuarkUploadTask, query: string) => {
  const host = String(task.upload_url || '').replace(/^https?:\/\//, '').replace(/\/+$/, '')
  if (!host || !task.bucket || !task.obj_key) return ''
  return `https://${task.bucket}.${host}/${task.obj_key}?${query}`
}

const buildCompleteXml = (parts: Array<{ number: number; etag: string }>) => `<?xml version="1.0" encoding="UTF-8"?><CompleteMultipartUpload>${parts.map((part) => `<Part><PartNumber>${part.number}</PartNumber><ETag>${part.etag}</ETag></Part>`).join('')}</CompleteMultipartUpload>`

const callbackToBase64 = (callback: unknown) => Buffer.from(typeof callback === 'string' ? callback : JSON.stringify(callback || {})).toString('base64')

const buildAuth = async (userId: string, task: QuarkUploadTask, method: 'PUT' | 'POST', contentType: string, query: string, options: { contentMd5?: string; callback?: string; hashContext?: string } = {}) => {
  const date = new Date().toUTCString()
  const hashHeader = options.hashContext ? `X-Oss-Hash-Ctx:${options.hashContext}\n` : ''
  const callbackHeader = options.callback ? `x-oss-callback:${options.callback}\n` : ''
  const authMeta = `${method}\n${options.contentMd5 || ''}\n${contentType}\n${date}\n${hashHeader}${callbackHeader}x-oss-date:${date}\nx-oss-user-agent:${OSS_USER_AGENT}\n/${task.bucket}/${task.obj_key}?${query}`
  const auth = await requestData(userId, 'file/upload/auth', { task_id: task.task_id || '', auth_info: task.auth_info || '', auth_meta: authMeta })
  if (!auth.auth_key) throw new Error('夸克网盘未返回上传授权')
  return { date, authorization: auth.auth_key }
}

const shouldRetry = (error: unknown) => error instanceof TypeError || (error instanceof Error && /timeout|network|fetch|reset|eof/i.test(error.message))

const parseExistingPartEtag = (body: string) => body.match(/<PartEtag>\s*"?([^<"]+)/i)?.[1]?.trim() || ''

class Sha1Context {
  private h0 = 0x67452301
  private h1 = 0xefcdab89
  private h2 = 0x98badcfe
  private h3 = 0x10325476
  private h4 = 0xc3d2e1f0
  private tail: Buffer<ArrayBufferLike> = Buffer.alloc(0)
  private bytes = 0n

  update(data: Buffer) {
    this.bytes += BigInt(data.length)
    let input = this.tail.length ? Buffer.concat([this.tail, data]) : data
    let offset = 0
    while (offset + 64 <= input.length) {
      this.process(input, offset)
      offset += 64
    }
    this.tail = input.subarray(offset)
  }

  toHeader(): HashContext {
    const bits = this.bytes * 8n
    return {
      hash_type: 'sha1',
      h0: String(this.h0 >>> 0), h1: String(this.h1 >>> 0), h2: String(this.h2 >>> 0), h3: String(this.h3 >>> 0), h4: String(this.h4 >>> 0),
      Nl: String(Number(bits & 0xffffffffn)), Nh: String(Number(bits >> 32n)), data: '', num: '0'
    }
  }

  private process(data: Buffer, offset: number) {
    const words = new Uint32Array(80)
    for (let index = 0; index < 16; index++) words[index] = data.readUInt32BE(offset + index * 4)
    for (let index = 16; index < 80; index++) words[index] = this.rotateLeft(words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16], 1)
    let a = this.h0; let b = this.h1; let c = this.h2; let d = this.h3; let e = this.h4
    for (let index = 0; index < 80; index++) {
      const f = index < 20 ? (b & c) | (~b & d) : index < 40 ? b ^ c ^ d : index < 60 ? (b & c) | (b & d) | (c & d) : b ^ c ^ d
      const k = index < 20 ? 0x5a827999 : index < 40 ? 0x6ed9eba1 : index < 60 ? 0x8f1bbcdc : 0xca62c1d6
      const next = (this.rotateLeft(a, 5) + f + e + k + words[index]) >>> 0
      e = d; d = c; c = this.rotateLeft(b, 30); b = a; a = next
    }
    this.h0 = (this.h0 + a) >>> 0; this.h1 = (this.h1 + b) >>> 0; this.h2 = (this.h2 + c) >>> 0; this.h3 = (this.h3 + d) >>> 0; this.h4 = (this.h4 + e) >>> 0
  }

  private rotateLeft(value: number, count: number) { return (value << count) | (value >>> (32 - count)) }
}

const hashFile = async (filePath: string, size: number, fileui: IUploadingUI) => {
  const opened = await OpenFileHandle(filePath)
  if (opened.error || !opened.handle) throw new Error(opened.error || '打开文件失败')
  const md5 = crypto.createHash('md5')
  const sha1 = crypto.createHash('sha1')
  try {
    let offset = 0
    while (offset < size) {
      if (!fileui.IsRunning) throw new Error('已暂停')
      const buffer = Buffer.alloc(Math.min(1024 * 1024, size - offset))
      const read = await opened.handle.read(buffer, 0, buffer.length, offset)
      const chunk = buffer.subarray(0, read.bytesRead)
      if (!chunk.length) throw new Error('读取文件失败')
      md5.update(chunk); sha1.update(chunk); offset += chunk.length
    }
  } finally {
    await opened.handle.close().catch(() => {})
  }
  return { md5: md5.digest('hex'), sha1: sha1.digest('hex') }
}

export default class QuarkUploadDisk {
  static async UploadOneFile(fileui: IUploadingUI): Promise<string> {
    const filePath = path.join(fileui.localFilePath, fileui.File.partPath)
    const name = fileui.File.name
    const mimeType = getMimeType(name)
    const parentId = fileui.parent_file_id.includes('root') ? '0' : fileui.parent_file_id
    fileui.Info.uploadState = 'hashing'
    let hashes: { md5: string; sha1: string }
    try { hashes = await hashFile(filePath, fileui.File.size, fileui) } catch (error: any) { return error?.message || '计算文件哈希失败' }
    if (!fileui.IsRunning) return '已暂停'

    fileui.Info.uploadState = 'running'
    let task: QuarkUploadTask
    try {
      task = await requestData(fileui.user_id, 'file/upload/pre', { ccp_hash_update: true, parallel_upload: true, dir_name: '', file_name: name, format_type: mimeType, l_created_at: Date.now(), l_updated_at: Date.now(), pdir_fid: parentId || '0', size: fileui.File.size })
      if (!task.task_id || !task.upload_id || !task.obj_key || !task.bucket || !task.upload_url) return '夸克网盘未返回完整上传任务'
    } catch (error: any) { return error?.message || '夸克网盘上传初始化失败' }

    const partSize = Number(task.part_size) > 0 ? Number(task.part_size) : DEFAULT_PART_SIZE
    const opened = await OpenFileHandle(filePath)
    if (opened.error || !opened.handle) return opened.error || '打开文件失败'
    const parts: Array<{ number: number; etag: string }> = []
    const sha1Context = new Sha1Context()
    try {
      let offset = 0
      let number = 1
      while (offset < fileui.File.size) {
        if (!fileui.IsRunning) return '已暂停'
        const buffer = Buffer.alloc(Math.min(partSize, fileui.File.size - offset))
        const read = await opened.handle.read(buffer, 0, buffer.length, offset)
        const body = buffer.subarray(0, read.bytesRead)
        if (!body.length) return '读取文件失败'
        const hashContext = number > 1 ? Buffer.from(JSON.stringify(sha1Context.toHeader())).toString('base64') : ''
        const query = `partNumber=${number}&uploadId=${encodeURIComponent(task.upload_id)}`
        const url = toOssUrl(task, query)
        if (!url) return '夸克网盘上传地址不完整'
        let etag = ''
        for (let attempt = 0; attempt <= 3; attempt++) {
          try {
            const auth = await buildAuth(fileui.user_id, task, 'PUT', mimeType, query, { hashContext })
            const headers: Record<string, string> = { Authorization: auth.authorization, 'Content-Type': mimeType, Referer: 'https://pan.quark.cn/', 'x-oss-date': auth.date, 'x-oss-user-agent': OSS_USER_AGENT }
            if (hashContext) headers['X-Oss-Hash-Ctx'] = hashContext
            const response = await fetch(url, { method: 'PUT', headers, body, signal: AbortSignal.timeout(PART_TIMEOUT) })
            if (response.ok) etag = response.headers.get('etag')?.replace(/"/g, '') || ''
            else if (response.status === 409) etag = parseExistingPartEtag(await response.text())
            if (etag) break
            if (response.status < 500 && response.status !== 408 && response.status !== 429) return `夸克网盘分片 ${number} 上传失败 (${response.status})`
          } catch (error) {
            if (!shouldRetry(error)) return error instanceof Error ? error.message : `夸克网盘分片 ${number} 上传失败`
          }
          if (attempt < 3) await Sleep(1000 * (2 ** attempt))
        }
        if (!etag) return `夸克网盘分片 ${number} 上传失败`
        sha1Context.update(body)
        parts.push({ number, etag })
        offset += body.length
        AliUploadDisk.RecordUploadProgress(fileui.UploadID, body.length, offset)
        number += 1
      }
    } finally {
      await opened.handle.close().catch(() => {})
    }

    try {
      const hashResult = await requestData(fileui.user_id, 'file/update/hash', { task_id: task.task_id, md5: hashes.md5, sha1: hashes.sha1 })
      if (hashResult.finish) {
        const finish = await requestData(fileui.user_id, 'file/upload/finish', { task_id: task.task_id, obj_key: task.obj_key })
        const fileId = String(finish.fid || finish.file?.fid || '')
        if (!fileId) return '夸克网盘上传完成但未返回文件 ID'
        fileui.File.uploaded_file_id = fileId
        fileui.File.uploaded_is_rapid = true
        return 'success'
      }
      const xml = buildCompleteXml(parts)
      const callback = callbackToBase64(task.callback)
      const contentMd5 = crypto.createHash('md5').update(xml).digest('base64')
      const query = `uploadId=${encodeURIComponent(task.upload_id)}`
      const url = toOssUrl(task, query)
      if (!url) return '夸克网盘上传地址不完整'
      const auth = await buildAuth(fileui.user_id, task, 'POST', 'application/xml', query, { contentMd5, callback })
      const response = await fetch(url, { method: 'POST', headers: { Authorization: auth.authorization, 'Content-MD5': contentMd5, 'Content-Type': 'application/xml', Referer: 'https://pan.quark.cn/', 'x-oss-callback': callback, 'x-oss-date': auth.date, 'x-oss-user-agent': OSS_USER_AGENT }, body: xml, signal: AbortSignal.timeout(COMMIT_TIMEOUT) })
      if (response.status !== 200) return `夸克网盘合并分片失败 (${response.status})`
      const finish = await requestData(fileui.user_id, 'file/upload/finish', { task_id: task.task_id, obj_key: task.obj_key })
      const fileId = String(finish.fid || finish.file?.fid || '')
      if (!fileId) return '夸克网盘上传完成但未返回文件 ID'
      fileui.File.uploaded_file_id = fileId
      fileui.File.uploaded_is_rapid = false
      return 'success'
    } catch (error: any) {
      return error?.message || '夸克网盘完成上传失败'
    }
  }
}
