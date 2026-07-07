import type { MediaServerConfig } from '../types/mediaServer'
import type { PlexPin, PlexResource } from '../types/mediaServerPlex'
import { openExternal } from '../utils/electronhelper'

const PLEX_CLIENT_ID_STORAGE_KEY = ''
const PLEX_PRODUCT = 'BoxPlayer'
const PLEX_VERSION = '1.7.20'
const PLEX_PLATFORM = 'BoxPlayer_Electron'
const PLEX_DEVICE = 'XbyBoxPlayer'
const POLL_INTERVAL_MS = 1000
const POLL_TIMEOUT_MS = 120_000

const createFallbackPlexClientId = () => `boxplayer-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`

export const getPlexClientId = () => {
  try {
    const storage = globalThis.localStorage
    const existing = storage?.getItem(PLEX_CLIENT_ID_STORAGE_KEY)?.trim()
    if (existing) return existing

    const generated = typeof globalThis.crypto?.randomUUID === 'function'
      ? `boxplayer-${globalThis.crypto.randomUUID()}`
      : createFallbackPlexClientId()
    storage?.setItem(PLEX_CLIENT_ID_STORAGE_KEY, generated)
    return generated
  } catch {
    return createFallbackPlexClientId()
  }
}

const createPin = async (): Promise<PlexPin> => {
  const response = await fetch('https://plex.tv/api/v2/pins?strong=true', {
    method: 'POST',
    headers: {
      Accept: 'application/json',
      'X-Plex-Product': PLEX_PRODUCT,
      'X-Plex-Client-Identifier': getPlexClientId()
    }
  })
  if (!response.ok) {
    throw new Error(`Plex 创建 PIN 失败 (${response.status})`)
  }
  return await response.json() as PlexPin
}

const buildAuthUrl = (pin: PlexPin) => {
  const clientId = getPlexClientId()
  const params = new URLSearchParams({
    clientID: clientId,
    code: pin.code,
    'context[device][product]': PLEX_PRODUCT,
    'context[device][version]': PLEX_VERSION,
    'context[device][platform]': PLEX_PLATFORM,
    'context[device][device]': PLEX_DEVICE
  })
  return `https://app.plex.tv/auth/#?${params.toString()}`
}

const checkPin = async (id: number, code: string): Promise<PlexPin> => {
  const response = await fetch(`https://plex.tv/api/v2/pins/${id}?code=${encodeURIComponent(code)}`, {
    headers: {
      Accept: 'application/json',
      'X-Plex-Device': PLEX_DEVICE,
      'X-Plex-Platform': PLEX_PLATFORM,
      'X-Plex-Product': PLEX_PRODUCT,
      'X-Plex-Client-Identifier': getPlexClientId()
    }
  })
  if (!response.ok) {
    throw new Error(`Plex 轮询登录状态失败 (${response.status})`)
  }
  return await response.json() as PlexPin
}

const pollForToken = async (pin: PlexPin) => {
  const deadline = Date.now() + POLL_TIMEOUT_MS
  while (Date.now() < deadline) {
    const latestPin = await checkPin(pin.id, pin.code)
    if (latestPin.authToken) return latestPin.authToken
    await new Promise((resolve) => window.setTimeout(resolve, POLL_INTERVAL_MS))
  }
  throw new Error('Plex 登录超时，请重试')
}

const fetchResources = async (token: string): Promise<PlexResource[]> => {
  const response = await fetch('https://plex.tv/api/v2/resources?includeRelay=1&includeHttps=1', {
    headers: {
      Accept: 'application/json',
      'X-Plex-Token': token,
      'X-Plex-Client-Identifier': getPlexClientId()
    }
  })
  if (!response.ok) {
    throw new Error(`Plex 获取资源列表失败 (${response.status})`)
  }
  return (await response.json() as PlexResource[])
    .filter((resource) => {
      const provides = resource.provides?.toLowerCase() || ''
      const product = resource.product?.toLowerCase() || ''
      const isPmsServer = provides.split(',').map((item) => item.trim()).includes('server') || product.includes('plex media server')
      return isPmsServer && Array.isArray(resource.connections) && resource.connections.length > 0 && !!resource.accessToken
    })
}

export const signInPlex = async (): Promise<PlexResource[]> => {
  const pin = await createPin()
  openExternal(buildAuthUrl(pin))
  const authToken = await pollForToken(pin)
  return await fetchResources(authToken)
}

type PreferredConnection = {
  uri: string
  address: string
  protocol: string
  port: string
  backupAddresses: Record<string, string>
}

const getPreferredConnection = (resource: PlexResource): PreferredConnection | null => {
  let local: PreferredConnection | null = null
  let relay: PreferredConnection | null = null
  let remote: PreferredConnection | null = null
  const backupAddresses: Record<string, string> = {}
  let remoteIndex = 0

  for (const connection of resource.connections) {
    const uri = connection.uri?.trim()
    const address = connection.address?.trim()
    const protocol = connection.protocol?.trim()
    const port = connection.port
    if (!uri || !address || !protocol || !port) continue

    const normalized = {
      uri,
      address,
      protocol,
      port: String(port),
      backupAddresses
    }

    if (connection.local) {
      backupAddresses.Local = uri
      local = normalized
      continue
    }
    if (connection.relay) {
      backupAddresses.Relay = uri
      relay = normalized
      continue
    }
    remoteIndex += 1
    if (backupAddresses.Remote) {
      backupAddresses[`Remote_${remoteIndex}`] = uri
    } else {
      backupAddresses.Remote = uri
      remote = normalized
    }
  }

  const preferred = local || remote || relay
  if (!preferred) return null
  return preferred
}

const normalizePlexBaseUrl = (connection: PreferredConnection) => {
  try {
    return new URL(connection.uri).toString().replace(/\/+$/, '')
  } catch {
    return `${connection.protocol}://${connection.address}${connection.port ? `:${connection.port}` : ''}`
  }
}

export const createPlexServerConfigs = (resources: PlexResource[]): Omit<MediaServerConfig, 'id' | 'createdAt' | 'updatedAt' | 'lastUsedAt'>[] => {
  return resources.flatMap((resource) => {
    const token = resource.accessToken?.trim()
    const preferred = getPreferredConnection(resource)
    if (!token || !preferred) return []

    const baseUrl = normalizePlexBaseUrl(preferred)
    return [{
      type: 'plex' as const,
      name: resource.name,
      baseUrl,
      notes: '',
      host: preferred.address,
      port: preferred.port,
      path: '',
      username: '',
      password: '',
      useHttps: preferred.protocol === 'https',
      syncFlag: true,
      backupAddresses: preferred.backupAddresses,
      accessToken: token,
      deviceId: getPlexClientId(),
      userId: `plex_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
      selectedResourceId: resource.clientIdentifier || resource.name,
      selectedResourceName: resource.name
    }]
  })
}

export const verifyPlexServerConfig = async (config: Pick<MediaServerConfig, 'baseUrl' | 'accessToken'>) => {
  const headers: Record<string, string> = {
    Accept: 'application/json',
    'X-Plex-Token': config.accessToken || '',
    'X-Plex-Product': PLEX_PRODUCT,
    'X-Plex-Client-Identifier': getPlexClientId()
  }
  for (const path of ['/identity', '/']) {
    try {
      const response = await fetch(`${config.baseUrl.replace(/\/+$/, '')}${path}`, { headers })
      if (!response.ok) continue
      const payload = await response.json() as any
      const container = payload.MediaContainer || payload
      const serverName = container.friendlyName || container.machineIdentifier || container.identifier || container.name
      if (serverName) return serverName as string
    } catch {
      // try the next Plex identity endpoint
    }
  }
  throw new Error('Plex 服务器连接验证失败')
}
