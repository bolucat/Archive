import { ENGINE_RPC_HOST, ENGINE_RPC_PORT, EMPTY_STRING } from '@shared/constants'
import { formatOptionsForEngine } from '@shared/utils'
import logger from './Logger'

export interface EngineClientOptions {
  host?: string
  port?: number
  secret?: string
}

export default class EngineClient {
  options: Required<EngineClientOptions>
  client!: any

  constructor (options: EngineClientOptions = {}) {
    this.options = {
      host: options.host || ENGINE_RPC_HOST,
      port: options.port || ENGINE_RPC_PORT,
      secret: options.secret ?? EMPTY_STRING
    }
  }

  init (): void { this.connect() }

  connect (): void {
    const { host, port, secret } = this.options
    ;(globalThis as any).self ||= globalThis
    const runtimeRequire = eval('require') as NodeRequire
    const Aria2 = runtimeRequire('aria2-lib')
    this.client = new Aria2({ host, port, secret } as any)
  }

  async call (method: string, ...args: any[]): Promise<any> {
    return (this.client as any).call(method, ...args).catch((err: any) => {
      logger.warn(`[motrix] aria2 RPC ${method} fail: ${err?.message}`)
    })
  }

  async changeGlobalOption (options: Record<string, any>): Promise<any> {
    const args = formatOptionsForEngine(options)
    return this.call('changeGlobalOption', args)
  }

  async shutdown (options: { force?: boolean } = {}): Promise<any> {
    const method = options.force ? 'forceShutdown' : 'shutdown'
    return this.call(method)
  }
}
