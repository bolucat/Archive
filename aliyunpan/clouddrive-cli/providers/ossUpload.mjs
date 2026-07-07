import { hmacSha1Base64 } from './uploadUtils.mjs'

function normalizeEndpoint(endpoint) {
  if (!endpoint) return ''
  if (endpoint.startsWith('http://') || endpoint.startsWith('https://')) return endpoint
  return `https://${endpoint}`
}

function hostInfo(endpoint, bucket) {
  const url = new URL(normalizeEndpoint(endpoint))
  const host = url.hostname.startsWith(`${bucket}.`) ? url.hostname : `${bucket}.${url.hostname}`
  return { protocol: url.protocol, host }
}

function canonicalHeaders(headers) {
  const keys = Object.keys(headers).map((key) => key.toLowerCase()).filter((key) => key.startsWith('x-oss-')).sort()
  return keys.map((key) => `${key}:${headers[key] ?? headers[key.toLowerCase()]}`).join('\n') + (keys.length ? '\n' : '')
}

function canonicalResource(bucket, object, params = {}) {
  let resource = `/${bucket}/${object}`
  const keys = Object.keys(params).filter((key) => params[key] !== undefined).sort()
  if (keys.length) resource += `?${keys.map((key) => (params[key] ? `${key}=${params[key]}` : key)).join('&')}`
  return resource
}

function sign(method, bucket, object, headers, params, secret) {
  const text = `${method}\n${headers['Content-MD5'] || ''}\n${headers['Content-Type'] || ''}\n${headers.Date || ''}\n${canonicalHeaders(headers)}${canonicalResource(bucket, object, params)}`
  return hmacSha1Base64(secret, text)
}

async function ossRequest(method, cred, bucket, object, { params = {}, body, contentType = '', callback } = {}) {
  const { protocol, host } = hostInfo(cred.endpoint, bucket)
  const headers = {
    Date: new Date().toUTCString(),
    'Content-Type': contentType,
  }
  if (body) headers['Content-Length'] = String(body.length)
  if (cred.securityToken) headers['x-oss-security-token'] = cred.securityToken
  if (callback?.callback) headers['x-oss-callback'] = callback.callback
  if (callback?.callback_var) headers['x-oss-callback-var'] = callback.callback_var
  headers.Authorization = `OSS ${cred.accessKeyId}:${sign(method, bucket, object, headers, params, cred.accessKeySecret)}`
  const query = Object.keys(params).map((key) => params[key] ? `${key}=${encodeURIComponent(params[key])}` : key).join('&')
  const resp = await fetch(`${protocol}//${host}/${object}${query ? `?${query}` : ''}`, { method, headers, body })
  const text = await resp.text().catch(() => '')
  return { status: resp.status, body: text, etag: resp.headers.get('etag') || '' }
}

export async function ossInitiateMultipart(cred, bucket, object, callback) {
  return ossRequest('POST', cred, bucket, object, { params: { uploads: '' }, contentType: 'application/xml', callback })
}

export async function ossUploadPart(cred, bucket, object, uploadId, partNumber, body) {
  return ossRequest('PUT', cred, bucket, object, {
    params: { partNumber: String(partNumber), uploadId },
    body,
    contentType: 'application/octet-stream',
  })
}

export async function ossCompleteMultipart(cred, bucket, object, uploadId, parts, callback) {
  const xml = Buffer.from(`<CompleteMultipartUpload>${parts.map((part) => `<Part><PartNumber>${part.partNumber}</PartNumber><ETag>${part.etag}</ETag></Part>`).join('')}</CompleteMultipartUpload>`)
  return ossRequest('POST', cred, bucket, object, {
    params: { uploadId },
    body: xml,
    contentType: 'application/xml',
    callback,
  })
}
