import axios from 'axios'
import { isEmpty } from 'lodash'
import { MAX_BT_TRACKER_LENGTH, ONE_SECOND, PROXY_SCOPES } from '@shared/constants'
import type { ProxyScopeType } from '@shared/constants'

export interface ProxyConfig {
  enable?: boolean
  server?: string
  bypass?: string
  scope?: ProxyScopeType[]
}

export const convertToAxiosProxy = (proxyServer: string): any => {
  if (!proxyServer) return undefined
  try {
    const url = new URL(proxyServer)
    return {
      protocol: url.protocol.replace(':', ''),
      host: url.hostname,
      port: Number(url.port) || 80,
      auth: url.username ? { username: url.username, password: url.password } : undefined
    }
  } catch { return undefined }
}

export const fetchBtTrackerFromSource = async (
  source: string[] = [],
  proxyConfig: ProxyConfig = {}
): Promise<string[]> => {
  if (isEmpty(source)) return []
  const now = Date.now()
  const { enable, server, scope = [] } = proxyConfig
  const proxy =
    enable && server && scope.includes(PROXY_SCOPES.UPDATE_TRACKERS)
      ? convertToAxiosProxy(server)
      : undefined

  const promises = source.map(async (url) =>
    axios
      .get(`${url}?t=${now}`, { timeout: 30 * ONE_SECOND, proxy: proxy || false })
      .then((res) => res.data as string)
      .catch(() => '')
  )
  const results = await Promise.allSettled(promises)
  const data = results
    .filter((r): r is PromiseFulfilledResult<string> => r.status === 'fulfilled')
    .map((r) => r.value)
    .filter((s) => typeof s === 'string' && s.length > 0)
  return [...new Set(data)]
}

export const convertTrackerDataToLine = (arr: string[] = []): string => {
  if (!arr.length) return ''
  const merged = arr.join('\r\n').split(/[\r\n]+/).filter(Boolean)
  return merged.join('\r\n')
}

export const convertTrackerDataToComma = (arr: string[] = []): string =>
  convertTrackerDataToLine(arr).replace(/\r?\n/g, ',')

export const reduceTrackerString = (str: string): string => {
  if (!str || str.length <= MAX_BT_TRACKER_LENGTH) return str
  const parts = str.split(',')
  let out = ''
  for (const p of parts) {
    if ((out + ',' + p).length > MAX_BT_TRACKER_LENGTH) break
    out = out ? out + ',' + p : p
  }
  return out
}
