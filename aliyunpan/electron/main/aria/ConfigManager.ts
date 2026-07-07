import { app } from 'electron'
import is from 'electron-is'
import Store from 'electron-store'
import { isEmpty } from 'lodash'
import {
  APP_THEME, APP_RUN_MODE, ENGINE_RPC_PORT, ENGINE_MAX_CONCURRENT_DOWNLOADS,
  IP_VERSION, NGOSANG_TRACKERS_BEST_URL,
  NGOSANG_TRACKERS_BEST_CDN_URL, NGOSANG_TRACKERS_BEST_IP_CDN_URL
} from '@shared/constants'
import { CHROME_UA } from '@shared/ua'
import { userKeys, systemKeys } from '@shared/configKeys'
import { separateConfig } from '@shared/utils'
import { reduceTrackerString } from '@shared/utils/tracker'
import { getConfigBasePath, getDhtPath, getMaxConnectionPerServer, getUserDownloadsPath } from './utils'
import logger from './Logger'

export default class ConfigManager {
  systemConfig!: InstanceType<typeof Store>
  userConfig!: InstanceType<typeof Store>

  constructor () { this.init() }

  init () {
    this.initUserConfig()
    this.initSystemConfig()
  }

  initSystemConfig () {
    this.systemConfig = new Store({
      name: 'system',
      cwd: getConfigBasePath(),
      defaults: {
        'all-proxy': '',
        'allow-overwrite': false,
        'auto-file-renaming': true,
        'bt-exclude-tracker': '',
        'bt-force-encryption': false,
        'bt-load-saved-metadata': true,
        'bt-save-metadata': true,
        'bt-tracker': '',
        'continue': true,
        'dht-file-path': getDhtPath(IP_VERSION.V4),
        'dht-file-path6': getDhtPath(IP_VERSION.V6),
        'dht-listen-port': 26701,
        'dir': getUserDownloadsPath(),
        'enable-dht6': true,
        'follow-metalink': true,
        'follow-torrent': true,
        'listen-port': 21301,
        'max-concurrent-downloads': ENGINE_MAX_CONCURRENT_DOWNLOADS,
        'max-connection-per-server': getMaxConnectionPerServer(),
        'max-download-limit': 0,
        'max-overall-download-limit': 0,
        'max-overall-upload-limit': 0,
        'no-proxy': '',
        'pause-metadata': false,
        'pause': true,
        'rpc-listen-port': ENGINE_RPC_PORT,
        'rpc-secret': 'S4znWTaZYQi3cpRNb',
        'seed-ratio': 2,
        'seed-time': 2880,
        'split': getMaxConnectionPerServer(),
        'user-agent': CHROME_UA
      }
    } as any)
    this.fixSystemConfig()
  }

  initUserConfig () {
    this.userConfig = new Store({
      name: 'user',
      cwd: getConfigBasePath(),
      defaults: {
        'auto-check-update': true,
        'auto-hide-window': false,
        'auto-sync-tracker': true,
        'cookie': '',
        'enable-upnp': true,
        'engine-bin-path': '',
        'engine-max-connection-per-server': getMaxConnectionPerServer(),
        'favorite-directories': [],
        'hide-app-menu': false,
        'history-directories': [],
        'keep-seeding': false,
        'keep-window-state': true,
        'last-check-update-time': 0,
        'last-sync-tracker-time': 0,
        'locale': app.getLocale(),
        'log-level': 'info',
        'open-at-login': false,
        'protocols': { mo: true, motrix: true, magnet: true, thunder: false },
        'proxy': { enable: false, server: '', bypass: '', scope: ['download'] },
        'resume-all-when-app-launched': false,
        'run-mode': APP_RUN_MODE.STANDARD,
        'show-progress-bar': true,
        'task-notification': true,
        'theme': APP_THEME.AUTO,
        'tracker-source': [NGOSANG_TRACKERS_BEST_IP_CDN_URL, NGOSANG_TRACKERS_BEST_CDN_URL],
        'tray-speedometer': true
      }
    } as any)
    this.fixUserConfig()
  }

  fixSystemConfig () {
    if (!this.systemConfig) return
    const all = this.systemConfig.store as Record<string, any>
    for (const k of Object.keys(all)) {
      if (!systemKeys.includes(k)) (this.systemConfig as any).delete(k)
    }
    const tracker = this.systemConfig.get('bt-tracker') as string
    if (tracker) this.systemConfig.set('bt-tracker', reduceTrackerString(tracker))
  }

  fixUserConfig () {
    if (!this.userConfig) return
    try {
      const settings = app.getLoginItemSettings()
      this.userConfig.set('open-at-login', !!settings.openAtLogin)
    } catch {}
    const src = this.userConfig.get('tracker-source') as string[]
    const motrixTrackerSources = [NGOSANG_TRACKERS_BEST_IP_CDN_URL, NGOSANG_TRACKERS_BEST_CDN_URL]
    if (isEmpty(src) || (src.length === 1 && src[0] === NGOSANG_TRACKERS_BEST_URL)) {
      this.userConfig.set('tracker-source', motrixTrackerSources)
    }
    if (this.userConfig.get('auto-sync-tracker') === false && !this.systemConfig?.get('bt-tracker')) {
      this.userConfig.set({
        'auto-sync-tracker': true,
        'last-sync-tracker-time': 0
      })
    }
    if (this.userConfig.get('enable-upnp') === false) this.userConfig.set('enable-upnp', true)
  }

  getSystemConfig (key?: string, def?: any): any {
    if (!key) return this.systemConfig.store
    return this.systemConfig.get(key, def)
  }

  getUserConfig (key?: string, def?: any): any {
    if (!key) return this.userConfig.store
    return this.userConfig.get(key, def)
  }

  getLocale (): string { return (this.userConfig.get('locale') as string) || app.getLocale() }

  setSystemConfig (...args: any[]): void {
    if (args.length === 1 && typeof args[0] === 'object') this.systemConfig.set(args[0])
    else (this.systemConfig.set as any)(args[0], args[1])
  }

  setUserConfig (...args: any[]): void {
    if (args.length === 1 && typeof args[0] === 'object') this.userConfig.set(args[0])
    else (this.userConfig.set as any)(args[0], args[1])
  }

  reset () {
    this.systemConfig.clear()
    this.userConfig.clear()
  }
}
