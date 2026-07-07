/**
 * HTTP request wrapper with cancellation support.
 * Adapted from lx-music-desktop request.js, using fetch API.
 */
export interface RequestResponse {
  statusCode: number
  body: any
  rawBody: string
}

export interface RequestObj {
  promise: Promise<any>
  cancelHttp: () => void
  isCancelled: boolean
}

const USER_AGENT =
  'Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/69.0.3497.100 Safari/537.36'

export function httpFetch(
  url: string,
  options: {
    method?: string
    headers?: Record<string, string>
    form?: Record<string, string>
    body?: string
    timeout?: number
  } = {}
): RequestObj {
  const controller = new AbortController()
  const timeoutMs = options.timeout || 15000
  const timeoutId = setTimeout(() => controller.abort(), timeoutMs)

  const headers: Record<string, string> = {
    'User-Agent': USER_AGENT,
    ...options.headers,
  }

  let body: string | undefined
  if (options.form) {
    body = new URLSearchParams(options.form).toString()
    headers['Content-Type'] = 'application/x-www-form-urlencoded'
  } else if (options.body) {
    body = options.body
  }

  const obj: RequestObj = {
    isCancelled: false,
    promise: Promise.resolve(undefined),
    cancelHttp: () => {
      obj.isCancelled = true
      controller.abort()
    },
  }

  obj.promise = fetch(url, {
    method: options.method || 'get',
    headers,
    body,
    signal: controller.signal,
  })
    .then(async (resp) => {
      clearTimeout(timeoutId)
      const rawBody = await resp.text()
      let parsedBody: any = rawBody
      try { parsedBody = JSON.parse(rawBody) } catch (_) {}
      return { statusCode: resp.status, body: parsedBody, rawBody }
    })
    .catch((err) => {
      clearTimeout(timeoutId)
      if (obj.isCancelled || err.name === 'AbortError') {
        return Promise.reject(new Error('请求被取消'))
      }
      if (err.message?.includes('fetch')) {
        return Promise.reject(new Error('网络连接失败'))
      }
      return Promise.reject(err)
    })

  return obj
}
