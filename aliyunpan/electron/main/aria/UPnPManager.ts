import logger from './Logger'

let client: any = null
const mappingStatus: Record<number, boolean> = {}

export default class UPnPManager {
  init (): void {
    if (client) return
    try {
      ;(globalThis as any).self ||= globalThis
      const runtimeRequire = eval('require') as NodeRequire
      const NatAPI = runtimeRequire(['@motrix', 'nat-api'].join('/'))
      client = new NatAPI({ autoUpdate: true })
    } catch (e: any) {
      logger.warn('[motrix] UPnP init failed: ' + e.message)
    }
  }

  map (port: number): Promise<void> {
    this.init()
    return new Promise((resolve, reject) => {
      if (!port) return reject(new Error('UPnP: port not specified'))
      if (!client) return resolve()
      try {
        client.map(port, (err: any) => {
          if (err) return reject(err.message || err)
          mappingStatus[port] = true
          resolve()
        })
      } catch (err: any) { reject(err.message || err) }
    })
  }

  unmap (port: number): Promise<void> {
    return new Promise((resolve, reject) => {
      if (!port || !mappingStatus[port] || !client) return resolve()
      try {
        client.unmap(port, (err: any) => {
          if (err) return reject(err.message || err)
          delete mappingStatus[port]
          resolve()
        })
      } catch (err: any) { reject(err.message || err) }
    })
  }

  closeClient (): Promise<void> {
    return new Promise((resolve) => {
      if (!client) return resolve()
      try {
        client.destroy(() => { client = null; resolve() })
      } catch { resolve() }
    })
  }
}
