import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { normalizeDrive115OssEndpoint, parseOssCallbackResult, parseOssError } from '../../cloud115/oss'

describe('115 upload resume', () => {
  it('uses HTTPS when the upload credential returns an HTTP OSS endpoint', () => {
    expect(normalizeDrive115OssEndpoint('http://oss.example.com')).toBe('https://oss.example.com')
    expect(normalizeDrive115OssEndpoint('oss.example.com')).toBe('https://oss.example.com')
  })

  it('extracts a service error from an OSS XML response', () => {
    expect(parseOssError('<Error><Code>InvalidPart</Code><Message>Part ETag does not match</Message></Error>')).toBe('InvalidPart: Part ETag does not match')
  })

  it('retains valid OSS callback file IDs and rejects callback failures', () => {
    expect(parseOssCallbackResult('{"code":0,"data":{"file_id":"115-file"}}')).toEqual({ fileId: '115-file', error: '' })
    expect(parseOssCallbackResult('{"code":200,"data":{"file_id":"115-file"}}')).toEqual({ fileId: '115-file', error: '' })
    expect(parseOssCallbackResult('{"state":false,"message":"callback denied"}')).toEqual({ fileId: '', error: 'callback denied' })
  })

  it('persists the multipart session and skips saved parts after resuming', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/cloud115/uploaddisk.ts'), 'utf8')

    expect(source).toContain('drive115_oss_upload_id')
    expect(source).toContain('const completedParts = new Map')
    expect(source).toContain('if (completedParts.has(partNumber))')
    expect(source).toContain('fileui.Info.drive115_oss_parts = Array.from(completedParts')
    expect(source).toContain('await DBUpload.saveUploadInfo(fileui.Info)')
  })
})
