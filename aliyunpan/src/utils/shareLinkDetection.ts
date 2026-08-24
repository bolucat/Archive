export type ShareLinkProvider = 'aliyun' | 'quark' | 'guangya' | 'pikpak' | 'baidu' | 'cloud123' | '115' | 'xunlei'

export interface DetectedShareLink {
  provider: ShareLinkProvider
  providerName: string
  url: string
  password: string
  canImport: boolean
}

const providerPatterns: { provider: ShareLinkProvider; providerName: string; canImport: boolean; pattern: RegExp }[] = [
  { provider: 'aliyun', providerName: '阿里云盘', canImport: true, pattern: /(?:https?:\/\/)?(?:www\.)?(?:(?:aliyundrive|alipan)\.com)\/s\/[0-9a-zA-Z_-]+[^\s]*/i },
  { provider: 'quark', providerName: '夸克网盘', canImport: true, pattern: /(?:https?:\/\/)?pan\.quark\.cn\/s\/[0-9a-zA-Z_-]+[^\s]*/i },
  { provider: 'guangya', providerName: '光鸭云盘', canImport: true, pattern: /(?:https?:\/\/)?(?:www\.)?guangyapan\.com\/s\/[0-9a-zA-Z_-]+[^\s]*/i },
  { provider: 'pikpak', providerName: 'PikPak', canImport: true, pattern: /(?:https?:\/\/)?(?:www\.)?(?:my)?pikpak\.com\/s\/[0-9a-zA-Z_-]+[^\s]*/i },
  { provider: 'baidu', providerName: '百度网盘', canImport: false, pattern: /(?:https?:\/\/)?pan\.baidu\.com\/(?:s\/[0-9a-zA-Z_-]+|share\/init\?surl=[0-9a-zA-Z_-]+)[^\s]*/i },
  { provider: 'cloud123', providerName: '123 云盘', canImport: false, pattern: /(?:https?:\/\/)?(?:www\.)?(?:123pan|123684|123912)\.com\/s\/[0-9a-zA-Z_-]+[^\s]*/i },
  { provider: '115', providerName: '115 网盘', canImport: false, pattern: /(?:https?:\/\/)?(?:115|115cdn)\.com\/s\/[0-9a-zA-Z_-]+[^\s]*/i },
  { provider: 'xunlei', providerName: '迅雷云盘', canImport: false, pattern: /(?:https?:\/\/)?pan\.xunlei\.com\/s\/[0-9a-zA-Z_-]+[^\s]*/i }
]

const trimUrlSuffix = (url: string): string => url.replace(/[)\]}>）】》。，,;!]+$/g, '')

const extractPassword = (text: string, url: string): string => {
  const query = url.match(/[?&#](?:pwd|password|passcode)=([0-9a-zA-Z]{4,8})/i)
  if (query?.[1]) return query[1]
  const label = text.match(/(?:提取码|密码|pwd|password)[^0-9a-zA-Z]{0,8}([0-9a-zA-Z]{4,8})/i)
  return label?.[1] || ''
}

export function detectShareLink(text: string): DetectedShareLink | undefined {
  const source = String(text || '').trim()
  if (!source) return undefined
  for (const entry of providerPatterns) {
    const match = source.match(entry.pattern)
    if (!match?.[0]) continue
    const rawUrl = trimUrlSuffix(match[0])
    const url = /^https?:\/\//i.test(rawUrl) ? rawUrl : `https://${rawUrl}`
    return { provider: entry.provider, providerName: entry.providerName, url, password: extractPassword(source, url), canImport: entry.canImport }
  }
  return undefined
}

export const canImportShareLink = (text: string): boolean => detectShareLink(text)?.canImport === true
