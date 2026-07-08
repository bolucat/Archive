import { request as httpsRequest } from 'https'

const PANHUB_ALLOWED_ORIGINS = ['https://api.xbyvideohub.com']

export interface PanHubRequestData {
  url: string
  method?: string
  headers?: Record<string, string>
  body?: string
}

export interface PanHubRequestResult {
  ok: boolean
  status: number
  data: unknown
}

type PanHubFetch = (input: string | URL, init?: RequestInit) => Promise<Response>

function requestWithHttps(url: URL, method: string, headers?: Record<string, string>, body?: string): Promise<PanHubRequestResult> {
  return new Promise((resolve, reject) => {
    const request = httpsRequest(url, { method, headers }, (response) => {
      const chunks: Buffer[] = []
      response.on('data', (chunk) => chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)))
      response.on('end', () => {
        const status = response.statusCode || 500
        const text = Buffer.concat(chunks).toString('utf8')
        let responseData: unknown = null
        if (text) {
          try {
            responseData = JSON.parse(text)
          } catch {
            responseData = { message: text }
          }
        }
        resolve({ ok: status >= 200 && status < 300, status, data: responseData })
      })
    })
    request.setTimeout(20000, () => request.destroy(new Error('PanHub 请求超时')))
    request.on('error', reject)
    if (method === 'POST' && body) request.write(body)
    request.end()
  })
}

export async function requestPanHub(data: PanHubRequestData, fetchImpl?: PanHubFetch): Promise<PanHubRequestResult> {
  const url = new URL(data.url)
  if (!PANHUB_ALLOWED_ORIGINS.includes(url.origin) || !url.pathname.startsWith('/api/')) throw new Error('不允许的 PanHub 请求地址')

  const method = (data.method || 'GET').toUpperCase()
  if (method !== 'GET' && method !== 'POST') throw new Error('不允许的 PanHub 请求方法')

  if (!fetchImpl) return requestWithHttps(url, method, data.headers, data.body)

  const response = await fetchImpl(url, {
    method,
    headers: data.headers,
    body: method === 'POST' ? data.body : undefined
  })
  const text = await response.text()
  let responseData: unknown = null
  if (text) {
    try {
      responseData = JSON.parse(text)
    } catch {
      responseData = { message: text }
    }
  }
  return { ok: response.ok, status: response.status, data: responseData }
}
