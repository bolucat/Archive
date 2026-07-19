export interface PanHubSearchLink {
  url: string
  password: string
  note: string
  datetime: string
  source?: string
}

export type PanHubMergedLinks = Record<string, PanHubSearchLink[]>

export interface PanHubSourceConfig {
  plugins: string[]
  channels: string[]
}

interface PanHubSearchOptions extends PanHubSourceConfig {
  apiBase: string
  keyword: string
  concurrency: number
  pluginTimeoutMs: number
  signal?: AbortSignal
  fetchImpl?: typeof fetch
  onProgress?: (merged: PanHubMergedLinks, total: number) => void
}

interface PanHubStreamOptions {
  apiBase: string
  keyword: string
  plugins?: string[]
  channels?: string[]
  concurrency?: number
  pluginTimeoutMs?: number
  signal?: AbortSignal
  fetchImpl?: typeof fetch
  onProgress?: (merged: PanHubMergedLinks, total: number) => void
  ipcRenderer?: {
    send: (channel: string, ...args: any[]) => void
    on: (channel: string, listener: (...args: any[]) => void) => void
    removeListener: (channel: string, listener: (...args: any[]) => void) => void
  }
}

export interface PanHubSearchRunResult {
  merged: PanHubMergedLinks
  total: number
  failedSources: number
  successfulSources: number
}

type PanHubIpcInvoke = (channel: string, data: unknown) => Promise<{ ok: boolean; status: number; data: unknown }>

const FALLBACK_PLUGINS = ['pansearch', 'qupansou', 'panta', 'hunhepan', 'jikepan', 'labi', 'thepiratebay', 'duoduo', 'xuexizhinan', 'nyaa']
const FALLBACK_CHANNELS = ['tgsearchers3', 'share_aliyun', 'Quark_Movies', 'NewQuark', 'alyp_4K_Movies', 'yunpanqk', 'BaiduCloudDisk', 'tianyifc']

function normalizeApiBase(apiBase: string): string {
  return apiBase.replace(/\/+$/, '')
}

function countMerged(merged: PanHubMergedLinks): number {
  return Object.values(merged).reduce((sum, items) => sum + items.length, 0)
}

export function createPanHubFetch(invoke?: PanHubIpcInvoke, fallbackFetch: typeof fetch = fetch): typeof fetch {
  if (!invoke) return fallbackFetch
  return (async (input: RequestInfo | URL, init?: RequestInit) => {
    if (init?.signal?.aborted) throw new DOMException('The operation was aborted.', 'AbortError')
    const headers = init?.headers ? Object.fromEntries(new Headers(init.headers).entries()) : undefined
    const request = invoke('PanHub:request', {
      url: input instanceof Request ? input.url : String(input),
      method: init?.method || 'GET',
      headers,
      body: typeof init?.body === 'string' ? init.body : undefined
    })
    const result = init?.signal
      ? await new Promise<Awaited<ReturnType<PanHubIpcInvoke>>>((resolve, reject) => {
          const abort = () => reject(new DOMException('The operation was aborted.', 'AbortError'))
          init.signal?.addEventListener('abort', abort, { once: true })
          request.then(resolve, reject).finally(() => init.signal?.removeEventListener('abort', abort))
        })
      : await request
    return new Response(JSON.stringify(result.data ?? null), {
      status: result.status,
      headers: { 'Content-Type': 'application/json' }
    })
  }) as typeof fetch
}

export async function discoverPanHubSources(apiBase: string, fetchImpl: typeof fetch = fetch, signal?: AbortSignal): Promise<PanHubSourceConfig> {
  try {
    const response = await fetchImpl(`${normalizeApiBase(apiBase)}/health`, { signal })
    if (!response.ok) throw new Error(`PanHub health request failed: HTTP ${response.status}`)
    const data = await response.json()
    const plugins = Array.isArray(data?.plugins) ? data.plugins.filter((item: unknown): item is string => typeof item === 'string' && !!item.trim()) : []
    const channels = Array.isArray(data?.channels) ? data.channels.filter((item: unknown): item is string => typeof item === 'string' && !!item.trim()) : []
    if (plugins.length || channels.length) return { plugins, channels }
  } catch (error: any) {
    if (error?.name === 'AbortError') throw error
  }
  return { plugins: [...FALLBACK_PLUGINS], channels: [...FALLBACK_CHANNELS] }
}

export function buildPanHubSearchUrls(apiBase: string, keyword: string, options: PanHubSourceConfig & { concurrency: number; pluginTimeoutMs: number }): string[] {
  const base = normalizeApiBase(apiBase)
  const concurrency = Math.min(16, Math.max(1, Math.round(options.concurrency || 4)))
  const common = {
    kw: keyword.trim(),
    res: 'merged_by_type',
    conc: String(concurrency),
    ext: JSON.stringify({ __plugin_timeout_ms: options.pluginTimeoutMs })
  }
  const urls: string[] = []

  for (const plugin of options.plugins) {
    const query = new URLSearchParams({ ...common, src: 'plugin', plugins: plugin })
    urls.push(`${base}/search?${query.toString()}`)
  }

  for (let index = 0; index < options.channels.length; index += concurrency) {
    const channels = options.channels.slice(index, index + concurrency).join(',')
    const query = new URLSearchParams({ ...common, src: 'tg', channels })
    urls.push(`${base}/search?${query.toString()}`)
  }
  return urls
}

export function extractPanHubMerged(data: any): PanHubMergedLinks {
  if (!data) return {}
  if (data.merged_by_type && typeof data.merged_by_type === 'object') return data.merged_by_type as PanHubMergedLinks

  const results = Array.isArray(data.results) ? data.results : Array.isArray(data) ? data : (data.items ?? data.list ?? data.data)
  if (!Array.isArray(results)) return {}

  const merged: PanHubMergedLinks = {}
  for (const result of results) {
    if (Array.isArray(result?.links)) {
      for (const link of result.links) {
        if (!link?.url) continue
        const type = link.type || 'others'
        ;(merged[type] ||= []).push({
          url: link.url,
          password: link.password || '',
          note: result.title || result.content || '',
          datetime: result.datetime || '',
          source: result.channel ? `tg:${result.channel}` : undefined
        })
      }
    } else if (result?.url) {
      const type = result.type || 'others'
      ;(merged[type] ||= []).push({
        url: result.url,
        password: result.password || '',
        note: result.note || '',
        datetime: result.datetime || '',
        source: result.source
      })
    }
  }
  return merged
}

export function mergePanHubMerged(target: PanHubMergedLinks, incoming: PanHubMergedLinks): PanHubMergedLinks {
  const merged: PanHubMergedLinks = { ...target }
  for (const [type, incomingItems] of Object.entries(incoming)) {
    const items = [...(merged[type] || [])]
    const seen = new Set(items.map((item) => item.url))
    for (const item of incomingItems || []) {
      if (!item?.url || seen.has(item.url)) continue
      seen.add(item.url)
      items.push(item)
    }
    merged[type] = items
  }
  return merged
}

export async function searchPanHubSources(options: PanHubSearchOptions): Promise<PanHubSearchRunResult> {
  const fetchImpl = options.fetchImpl || fetch
  const concurrency = Math.min(16, Math.max(1, Math.round(options.concurrency || 4)))
  const urls = buildPanHubSearchUrls(options.apiBase, options.keyword, options)
  let nextIndex = 0
  let failedSources = 0
  let successfulSources = 0
  let merged: PanHubMergedLinks = {}

  const worker = async () => {
    while (nextIndex < urls.length) {
      const url = urls[nextIndex++]
      try {
        const response = await fetchImpl(url, { signal: options.signal, credentials: 'include' })
        if (!response.ok) throw new Error(`PanHub search request failed: HTTP ${response.status}`)
        const payload = await response.json()
        if (payload?.code !== undefined && payload.code !== 0) throw new Error(payload.message || '搜索失败')
        successfulSources++
        const incoming = extractPanHubMerged(payload?.data ?? payload)
        if (Object.keys(incoming).length) {
          merged = mergePanHubMerged(merged, incoming)
          options.onProgress?.(merged, countMerged(merged))
        }
      } catch (error: any) {
        if (error?.name === 'AbortError') throw error
        failedSources++
      }
    }
  }

  await Promise.all(Array.from({ length: Math.min(concurrency, urls.length) }, () => worker()))
  return { merged, total: countMerged(merged), failedSources, successfulSources }
}

export async function searchPanHubStream(options: PanHubStreamOptions): Promise<PanHubSearchRunResult> {
  let merged: PanHubMergedLinks = {}
  let failedSources = 0
  let successfulSources = 0
  let receivedDone = false

  const consumeEvent = (event: any) => {
    if (event?.type === 'results') {
      successfulSources++
      const incoming = extractPanHubMerged(event.data)
      if (Object.keys(incoming).length) {
        merged = mergePanHubMerged(merged, incoming)
        options.onProgress?.(merged, countMerged(merged))
      }
    } else if (event?.type === 'warning') {
      failedSources++
    } else if (event?.type === 'done') {
      receivedDone = true
      const incoming = extractPanHubMerged(event.data)
      if (Object.keys(incoming).length) merged = mergePanHubMerged(merged, incoming)
    } else if (event?.type === 'error') {
      throw new Error(event.message || 'PanHub 流式搜索失败')
    }
  }

  const body = JSON.stringify({
    kw: options.keyword.trim(),
    src: 'all',
    res: 'merged_by_type'
  })
  const url = `${normalizeApiBase(options.apiBase)}/search/stream`

  if (options.ipcRenderer) {
    const requestId = globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(36).slice(2)}`
    await new Promise<void>((resolve, reject) => {
      const ipc = options.ipcRenderer!
      const matches = (message: any) => message?.requestId === requestId
      const cleanup = () => {
        ipc.removeListener('PanHub:stream-event', onEvent)
        ipc.removeListener('PanHub:stream-end', onEnd)
        ipc.removeListener('PanHub:stream-error', onError)
        options.signal?.removeEventListener('abort', onAbort)
      }
      const onEvent = (_ipcEvent: unknown, message: any) => {
        if (!matches(message)) return
        try {
          consumeEvent(message.payload)
        } catch (error) {
          ipc.send('PanHub:stream-cancel', requestId)
          cleanup()
          reject(error)
        }
      }
      const onEnd = (_ipcEvent: unknown, message: any) => {
        if (!matches(message)) return
        cleanup()
        if (!receivedDone) reject(new Error('PanHub 流式响应提前结束'))
        else resolve()
      }
      const onError = (_ipcEvent: unknown, message: any) => {
        if (!matches(message)) return
        cleanup()
        reject(new Error(String(message.payload || 'PanHub 流式搜索失败')))
      }
      const onAbort = () => {
        ipc.send('PanHub:stream-cancel', requestId)
        cleanup()
        reject(new DOMException('The operation was aborted.', 'AbortError'))
      }
      ipc.on('PanHub:stream-event', onEvent)
      ipc.on('PanHub:stream-end', onEnd)
      ipc.on('PanHub:stream-error', onError)
      options.signal?.addEventListener('abort', onAbort, { once: true })
      ipc.send('PanHub:stream-start', {
        requestId,
        request: { url, method: 'POST', headers: { 'Content-Type': 'application/json' }, body }
      })
    })
  } else {
    const response = await (options.fetchImpl || fetch)(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
      signal: options.signal,
      credentials: 'include'
    })
    if (!response.ok) throw new Error(`PanHub stream request failed: HTTP ${response.status}`)
    if (!response.body) throw new Error('PanHub stream response body is empty')
    const reader = response.body.getReader()
    const decoder = new TextDecoder()
    let buffer = ''
    while (true) {
      const { value, done } = await reader.read()
      if (done) break
      buffer += decoder.decode(value, { stream: true })
      const lines = buffer.split('\n')
      buffer = lines.pop() || ''
      for (const line of lines) if (line.trim()) consumeEvent(JSON.parse(line))
    }
    const tail = `${buffer}${decoder.decode()}`.trim()
    if (tail) consumeEvent(JSON.parse(tail))
    if (!receivedDone) throw new Error('PanHub stream response ended early')
  }

  return { merged, total: countMerged(merged), failedSources, successfulSources: Math.max(successfulSources, receivedDone ? 1 : 0) }
}
