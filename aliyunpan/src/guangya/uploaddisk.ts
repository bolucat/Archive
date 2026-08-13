import crypto from 'crypto'
import { openAsBlob } from 'fs'
import path from 'path'
import { FileHandle } from 'fs/promises'
import { OpenFileHandle } from '../utils/filehelper'
import { IUploadingUI } from '../utils/dbupload'
import AliUploadDisk from '../aliapi/uploaddisk'
import { Sleep } from '../utils/format'
import { apiGuangyaCheckFlashUpload, apiGuangyaUploadBuffer, apiGuangyaUploadInfo, apiGuangyaUploadTaskFile, apiGuangyaUploadToken, GuangyaUploadTokenData } from './upload'

const SMALL_FILE_SIZE = 1024 * 1024

const shouldRetryFileOpen = (error: string) => error.includes('同时打开文件过多') || error.includes('文件被其他程序占用') || error.includes('操作超时') || error.includes('IO错误')

const openFileHandleWithRetry = async (filePath: string) => {
  let lastError = ''
  for (let i = 0; i < 5; i++) {
    const fh = await OpenFileHandle(filePath)
    if (!fh.error && fh.handle) return fh
    lastError = fh.error || '打开文件失败'
    if (!shouldRetryFileOpen(lastError) || i === 4) return { handle: undefined, error: lastError }
    await Sleep(400 * (i + 1))
  }
  return { handle: undefined, error: lastError || '打开文件失败' }
}

const readAll = async (filePath: string, size: number): Promise<{ buff?: Buffer; error: string }> => {
  const fh = await openFileHandleWithRetry(filePath)
  if (fh.error || !fh.handle) return { error: fh.error || '打开文件失败' }
  try {
    const buff = Buffer.alloc(size)
    const read = await fh.handle.read(buff, 0, size, 0)
    return { buff: buff.subarray(0, read.bytesRead), error: '' }
  } catch (error: any) {
    return { error: error?.message || '读取文件失败' }
  } finally {
    await fh.handle.close().catch(() => {})
  }
}

const getGcidChunkSize = (fileSize: number) => {
  if (fileSize <= 0x8000000) return 262144
  if (fileSize <= 0x10000000) return 524288
  if (fileSize <= 0x20000000) return 1048576
  return 2097152
}

const gcidFile = async (filePath: string, fileSize: number): Promise<{ gcid: string; error: string }> => {
  const fh = await openFileHandleWithRetry(filePath)
  if (fh.error || !fh.handle) return { gcid: '', error: fh.error || '打开文件失败' }
  try {
    const chunkSize = getGcidChunkSize(fileSize)
    const chunks: Buffer[] = []
    let pos = 0
    while (pos < fileSize) {
      const size = Math.min(chunkSize, fileSize - pos)
      const buff = Buffer.alloc(size)
      const read = await fh.handle.read(buff, 0, size, pos)
      const chunk = buff.subarray(0, read.bytesRead)
      chunks.push(crypto.createHash('sha1').update(chunk).digest())
      pos += chunk.length
    }
    return { gcid: crypto.createHash('sha1').update(Buffer.concat(chunks)).digest('hex').toUpperCase(), error: '' }
  } catch (error: any) {
    return { gcid: '', error: error?.message || '计算 GCID 失败' }
  } finally {
    await fh.handle.close().catch(() => {})
  }
}

export default class GuangyaUploadDisk {
  static async UploadOneFile(fileui: IUploadingUI): Promise<string> {
    const filePath = path.join(fileui.localFilePath, fileui.File.partPath)
    fileui.Info.uploadState = 'hashing'

    if (fileui.File.size < SMALL_FILE_SIZE) {
      const { buff, error } = await readAll(filePath, fileui.File.size)
      if (!buff) return error || '读取文件失败'
      fileui.Info.uploadState = 'running'
      const result = await apiGuangyaUploadBuffer(fileui.user_id, fileui.parent_file_id, fileui.File.name, buff)
      if (!result.file_id) return result.error || '光鸭云盘上传失败'
      fileui.File.uploaded_file_id = result.file_id
      fileui.File.uploaded_is_rapid = false
      AliUploadDisk.RecordUploadProgress(fileui.UploadID, fileui.File.size, fileui.File.size)
      return 'success'
    }

    const { gcid, error } = await gcidFile(filePath, fileui.File.size)
    fileui.Info.uploadState = 'running'
    if (!gcid) return error || '计算 GCID 失败'

    const tokenResp = await apiGuangyaUploadToken(fileui.user_id, fileui.File.name, fileui.File.size, fileui.parent_file_id)
    if (!tokenResp.data) return tokenResp.error
    const flashResp = await apiGuangyaCheckFlashUpload(fileui.user_id, tokenResp.data.taskId, gcid)
    if (flashResp.canFlashUpload) {
      const info = await apiGuangyaUploadInfo(fileui.user_id, tokenResp.data.taskId)
      if (info.fileId) {
        fileui.File.uploaded_file_id = info.fileId
        fileui.File.uploaded_is_rapid = true
        AliUploadDisk.RecordUploadProgress(fileui.UploadID, fileui.File.size, fileui.File.size)
        return 'success'
      }
      return info.error || '光鸭云盘秒传失败'
    }

    const fileBlob = await openAsBlob(filePath, { type: 'application/octet-stream' })
    const uploadError = await apiGuangyaUploadTaskFile(tokenResp.data, fileui.File.name, fileBlob)
    if (uploadError) return uploadError
    for (let i = 0; i < 3; i++) {
      const info = await apiGuangyaUploadInfo(fileui.user_id, tokenResp.data.taskId)
      if (info.fileId) {
        fileui.File.uploaded_file_id = info.fileId
        fileui.File.uploaded_is_rapid = false
        return 'success'
      }
      if (!info.uploading && info.error) return info.error
      await Sleep(2000)
    }
    return '光鸭云盘上传完成确认超时'
  }
}
