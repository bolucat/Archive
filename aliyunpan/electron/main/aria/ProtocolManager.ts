import { EventEmitter } from 'node:events'
import { app } from 'electron'
import is from 'electron-is'
import { parse as parseQS } from 'node:querystring'
import { ADD_TASK_TYPE } from '@shared/constants'
import protocolMap from './configs/protocol'
import logger from './Logger'

export interface ProtocolManagerOptions {
  protocols?: Record<string, boolean>
}

export default class ProtocolManager extends EventEmitter {
  protocols: Record<string, boolean>

  constructor (options: ProtocolManagerOptions = {}) {
    super()
    this.protocols = {
      mo: true, motrix: true, magnet: true, thunder: false,
      ...(options.protocols || {})
    }
  }

  init (): void { this.setup(this.protocols) }

  setup (protocols: Record<string, boolean>): void {
    if ((is as any).dev?.() || (is as any).masBuild?.()) return
    Object.entries(protocols).forEach(([scheme, enabled]) => {
      if (enabled) app.setAsDefaultProtocolClient(scheme)
      else app.removeAsDefaultProtocolClient(scheme)
    })
  }

  handle (url: string): void {
    if (!url) return
    const lower = url.toLowerCase()
    if (/^https?:|^ftp:|^magnet:|^thunder:/i.test(lower)) {
      this.handleResourceProtocol(url)
    } else if (/^mo:|^motrix:/i.test(lower)) {
      this.handleMoProtocol(url)
    } else {
      logger.warn('[motrix] unsupported protocol: ' + url)
    }
  }

  handleResourceProtocol (url: string): void {
    const app = (globalThis as any).motrixApplication
    if (!app) return
    app.sendCommandToAll('application:new-task', { type: ADD_TASK_TYPE.URI, uri: url })
  }

  handleMoProtocol (url: string): void {
    try {
      const parsed = new URL(url)
      const command = protocolMap[parsed.host]
      if (!command) return
      const args = parseQS(parsed.search.slice(1))
      const application = (globalThis as any).motrixApplication
      if (application) application.sendCommandToAll(command, args)
    } catch {}
  }
}
