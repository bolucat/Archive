import { join } from 'node:path'
import { existsSync, lstatSync } from 'node:fs'
import { app } from 'electron'
import {
  IS_PORTABLE, PORTABLE_EXECUTABLE_DIR,
  ENGINE_MAX_CONNECTION_PER_SERVER, IP_VERSION, RESOURCE_TAGS
} from '@shared/constants'
import { engineBinMap, engineArchMap } from './configs/engine'
import { getStaticPath } from '../utils/mainfile'

export const getUserDataPath = (): string => {
  if (IS_PORTABLE) return PORTABLE_EXECUTABLE_DIR
  return app.getPath('userData')
}

export const getUserDownloadsPath = (): string => app.getPath('downloads')
export const getConfigBasePath = (): string => getUserDataPath()
export const getSessionPath = (): string => join(getUserDataPath(), 'download.session')
export const getEnginePidPath = (): string => join(getUserDataPath(), 'engine.pid')

export const getDhtPath = (protocol: number): string =>
  join(getUserDataPath(), protocol === IP_VERSION.V6 ? 'dht6.dat' : 'dht.dat')

export const getEngineBin = (platform: string): string => engineBinMap[platform] || 'aria2c'
export const getEngineArch = (platform: string, arch: string): string =>
  engineArchMap[platform]?.[arch] || arch

export const getDevEnginePath = (platform: string, arch: string): string => {
  const archDir = getEngineArch(platform, arch)
  return join(getStaticPath('engine'), platform, archDir)
}

export const getProdEnginePath = (platform: string, arch: string): string => {
  const archDir = getEngineArch(platform, arch)
  return join(getStaticPath('engine'), platform, archDir)
}

export const getEnginePath = (platform: string, arch: string): string =>
  getProdEnginePath(platform, arch)

export const getAria2BinPath = (platform: string, arch: string): string =>
  join(getEnginePath(platform, arch), getEngineBin(platform))

export const getAria2ConfPath = (platform: string, arch: string): string =>
  join(getEnginePath(platform, arch), 'aria2.conf')

export const transformConfig = (config: Record<string, any>): string[] => {
  const result: string[] = []
  for (const [k, v] of Object.entries(config)) {
    if (v === '' || v === undefined || v === null) continue
    result.push(`--${k}=${v}`)
  }
  return result
}

export const getMaxConnectionPerServer = (): number => ENGINE_MAX_CONNECTION_PER_SERVER

export const checkIsSupportedSchema = (url: string): boolean => {
  const lower = (url || '').toLowerCase()
  return RESOURCE_TAGS.some((t) => lower.startsWith(t)) ||
    lower.startsWith('mo:') || lower.startsWith('motrix:')
}

export const isDirectory = (p: string): boolean => {
  try { return existsSync(p) && lstatSync(p).isDirectory() } catch { return false }
}

export const splitArgv = (argv: string[]): { extra: Record<string, string>; args: string[] } => {
  const extra: Record<string, string> = {}
  const args: string[] = []
  for (const a of argv) {
    if (a.startsWith('--')) {
      const [k, ...rest] = a.slice(2).split('=')
      extra[k] = rest.join('=')
    } else {
      args.push(a)
    }
  }
  return { extra, args }
}

export const parseArgvAsUrl = (argv: string[]): string | undefined => {
  if (argv.length < 2) return
  const u = argv[1]
  if (checkIsSupportedSchema(u)) return u
}

export const parseArgvAsFile = (argv: string[]): string | undefined => {
  if (argv.length < 2) return
  let f = argv[1]
  if (isDirectory(f)) return
  if (process.platform === 'linux' && f.startsWith('file://')) f = f.slice(7)
  return f
}
