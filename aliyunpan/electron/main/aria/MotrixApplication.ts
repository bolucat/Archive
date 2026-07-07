import { EventEmitter } from 'node:events'
import { app, ipcMain, BrowserWindow, dialog } from 'electron'
import { isEmpty } from 'lodash'
import { AUTO_SYNC_TRACKER_INTERVAL } from '@shared/constants'
import { checkIsNeedRun } from '@shared/utils'
import { fetchBtTrackerFromSource, convertTrackerDataToComma, reduceTrackerString } from '@shared/utils/tracker'
import ConfigManager from './ConfigManager'
import Engine from './Engine'
import EngineClient from './EngineClient'
import UPnPManager from './UPnPManager'
import EnergyManager from './EnergyManager'
import ProtocolManager from './ProtocolManager'
import Context from './Context'
import logger from './Logger'

export default class MotrixApplication extends EventEmitter {
  configManager!: ConfigManager
  engine!: Engine
  engineClient!: EngineClient
  upnp!: UPnPManager
  energyManager!: EnergyManager
  protocolManager!: ProtocolManager
  context!: Context
  private initialized = false
  private trackerTimer: ReturnType<typeof setInterval> | null = null

  async init (): Promise<void> {
    if (this.initialized) return
    this.initContext()
    this.initConfigManager()
    this.startEngine()
    this.initEngineClient()
    this.initUPnPManager()
    this.initEnergyManager()
    this.initProtocolManager()
    this.handleConfigChanges()
    this.handleIpc()
    ;(globalThis as any).motrixApplication = this
    this.initialized = true
    this.emit('application:initialized')
    setTimeout(() => this.afterInit(), 0)
  }

  initContext (): void { this.context = new Context() }
  initConfigManager (): void { this.configManager = new ConfigManager() }

  startEngine (): void {
    try {
      this.engine = new Engine({
        systemConfig: this.configManager.getSystemConfig(),
        userConfig: this.configManager.getUserConfig()
      })
      this.engine.start()
    } catch (err: any) {
      logger.error('[motrix] startEngine failed: ' + err.message)
      try {
        dialog.showMessageBox({ type: 'error', title: 'Engine Error', message: err.message })
      } catch {}
    }
  }

  async stopEngine (): Promise<void> {
    try { await this.engineClient?.shutdown({ force: true }) }
    catch (err: any) { logger.warn('[motrix] shutdown engine failed: ' + err.message) }
    setImmediate(() => { try { this.engine?.stop() } catch {} })
  }

  initEngineClient (): void {
    const port = this.configManager.getSystemConfig('rpc-listen-port')
    const secret = this.configManager.getSystemConfig('rpc-secret')
    this.engineClient = new EngineClient({ port, secret })
    this.engineClient.init()
  }

  initUPnPManager (): void {
    this.upnp = new UPnPManager()
    this.watchUPnPEnabledChange()
    this.watchUPnPPortsChange()
    if (this.configManager.getUserConfig('enable-upnp')) this.startUPnPMapping()
  }

  initEnergyManager (): void {
    this.energyManager = new EnergyManager()
    this.on('download-status-change', (downloading: boolean) => {
      if (downloading) this.energyManager.startPowerSaveBlocker()
      else this.energyManager.stopPowerSaveBlocker()
    })
  }

  initProtocolManager (): void {
    const protocols = this.configManager.getUserConfig('protocols') as Record<string, boolean>
    this.protocolManager = new ProtocolManager({ protocols })
    this.protocolManager.init()
  }

  async startUPnPMapping (): Promise<void> {
    const btPort = this.configManager.getSystemConfig('listen-port') as number
    const dhtPort = this.configManager.getSystemConfig('dht-listen-port') as number
    await Promise.allSettled([this.upnp.map(btPort), this.upnp.map(dhtPort)])
  }

  async stopUPnPMapping (): Promise<void> {
    const btPort = this.configManager.getSystemConfig('listen-port') as number
    const dhtPort = this.configManager.getSystemConfig('dht-listen-port') as number
    await Promise.allSettled([this.upnp.unmap(btPort), this.upnp.unmap(dhtPort)])
  }

  watchUPnPEnabledChange (): void {
    this.configManager.userConfig.onDidChange('enable-upnp', async (newValue: any) => {
      if (newValue) await this.startUPnPMapping()
      else { await this.stopUPnPMapping(); await this.upnp.closeClient() }
    })
  }

  watchUPnPPortsChange (): void {
    for (const key of ['listen-port', 'dht-listen-port']) {
      this.configManager.systemConfig.onDidChange(key, async (newValue: any, oldValue: any) => {
        if (!this.configManager.getUserConfig('enable-upnp')) return
        await Promise.allSettled([this.upnp.unmap(oldValue), this.upnp.map(newValue)])
      })
    }
  }

  handleConfigChanges (): void {
    this.configManager.userConfig.onDidChange('proxy', async (newValue: any) => {
      const { enable, server, bypass, scope = [] } = newValue || {}
      const system = enable && server && scope.includes('download')
        ? { 'all-proxy': server, 'no-proxy': bypass || '' } : {}
      this.configManager.setSystemConfig(system)
      try { await this.engineClient.call('changeGlobalOption', system) } catch {}
    })
  }

  handleIpc (): void {
    ipcMain.handle('motrix:get-app-config', async () => ({
      ...this.configManager.getSystemConfig(),
      ...this.configManager.getUserConfig(),
      ...this.context.get()
    }))
    ipcMain.handle('motrix:set-config', async (_event, config: { system?: any; user?: any }) => {
      await this.savePreference(config)
    })
    ipcMain.handle('motrix:get-trackers', async () =>
      this.configManager.getSystemConfig('bt-tracker') || ''
    )
    ipcMain.handle('motrix:sync-trackers', async () => {
      const source = this.configManager.getUserConfig('tracker-source') as string[]
      const proxy = this.configManager.getUserConfig('proxy', { enable: false })
      this.syncTrackers(source, proxy)
    })
    ipcMain.on('motrix:command', (_event, command: string, ...args: any[]) => {
      this.emit(command, ...args)
    })
    ipcMain.on('motrix:event', (_event, eventName: string, ...args: any[]) => {
      this.emit(eventName, ...args)
    })
    this.on('application:save-preference', (config: any) => this.savePreference(config))
  }

  async savePreference (config: any = {}): Promise<void> {
    const { system, user } = config || {}
    if (!isEmpty(system)) {
      this.configManager.setSystemConfig(system)
      try { await this.engineClient.changeGlobalOption(system) } catch {}
    }
    if (!isEmpty(user)) this.configManager.setUserConfig(user)
  }

  sendCommandToAll (command: string, ...args: any[]): void {
    BrowserWindow.getAllWindows().forEach((w) => {
      try { w.webContents.send('motrix:command', command, ...args) } catch {}
    })
  }

  afterInit (): void {
    this.autoSyncTrackers()
    this.autoResumeTask()
    this.trackerTimer = setInterval(() => this.autoSyncTrackers(), AUTO_SYNC_TRACKER_INTERVAL)
  }

  syncTrackers (source: string[], proxy: any): void {
    if (isEmpty(source)) return
    setTimeout(() => {
      fetchBtTrackerFromSource(source, proxy)
        .then((data) => {
          if (!data?.length) return
          let tracker = convertTrackerDataToComma(data)
          tracker = reduceTrackerString(tracker)
          this.savePreference({
            system: { 'bt-tracker': tracker },
            user: { 'last-sync-tracker-time': Date.now() }
          })
          logger.info('[motrix] BT trackers synced')
        })
        .catch((err) => logger.warn('[motrix] tracker sync error: ' + err?.message))
    }, 500)
  }

  autoSyncTrackers (): void {
    const enable = this.configManager.getUserConfig('auto-sync-tracker') as boolean
    const lastTime = this.configManager.getUserConfig('last-sync-tracker-time') as number
    if (!checkIsNeedRun(enable, lastTime, AUTO_SYNC_TRACKER_INTERVAL)) return
    const source = this.configManager.getUserConfig('tracker-source') as string[]
    const proxy = this.configManager.getUserConfig('proxy', { enable: false })
    this.syncTrackers(source, proxy)
  }

  autoResumeTask (): void {
    if (!this.configManager.getUserConfig('resume-all-when-app-launched')) return
    this.engineClient.call('unpauseAll').catch(() => undefined)
  }

  async quit (): Promise<void> {
    if (this.trackerTimer) { clearInterval(this.trackerTimer); this.trackerTimer = null }
    await this.stopEngine()
    try { await this.upnp?.closeClient() } catch {}
    try { this.energyManager?.stopPowerSaveBlocker() } catch {}
  }
}
