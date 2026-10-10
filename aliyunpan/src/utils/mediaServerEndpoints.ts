import type { MediaServerConfig } from '../types/mediaServer'

export function saveMediaServerEndpoint(server: Pick<MediaServerConfig, 'backupAddresses' | 'selectedLineName'>, oldName: string | null, name: string, input: string) {
  let parsed: URL
  try { parsed = new URL(input.trim()) } catch { throw new Error('请输入有效的 HTTP 或 HTTPS 地址') }
  if (!['http:', 'https:'].includes(parsed.protocol) || parsed.username || parsed.password || parsed.search || parsed.hash) throw new Error('端点需为 HTTP 或 HTTPS 地址，且不能包含账号、查询参数或片段')
  const key = name.trim() || parsed.host
  if (['__primary__', '__proto__', 'constructor', 'prototype'].includes(key)) throw new Error('请使用其他端点名称')
  const entries = Object.entries(server.backupAddresses || {})
  if (entries.some(([existing]) => existing !== oldName && existing.toLocaleLowerCase() === key.toLocaleLowerCase())) throw new Error('已存在同名端点')
  const backupAddresses = Object.fromEntries([...entries.filter(([existing]) => existing !== oldName), [key, parsed.toString().replace(/\/+$/, '')]])
  return { backupAddresses, selectedLineName: oldName && server.selectedLineName === oldName ? key : server.selectedLineName || '' }
}

export function removeMediaServerEndpoint(server: Pick<MediaServerConfig, 'backupAddresses' | 'selectedLineName'>, name: string) {
  return { backupAddresses: Object.fromEntries(Object.entries(server.backupAddresses || {}).filter(([key]) => key !== name)), selectedLineName: server.selectedLineName === name ? '' : server.selectedLineName || '' }
}
