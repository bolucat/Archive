import {
  getEnginePath, getAria2BinPath, getAria2ConfPath, getSessionPath, getEnginePidPath
} from './utils'

export interface MotrixContext {
  platform: string
  arch: string
  'session-path': string
  'engine-pid-path': string
  'engine-path': string
  'aria2-bin-path': string
  'aria2-conf-path': string
}

export default class Context {
  context!: MotrixContext

  constructor () { this.init() }

  init (): void {
    const platform = process.platform
    const arch = process.arch
    this.context = {
      platform, arch,
      'session-path': getSessionPath(),
      'engine-pid-path': getEnginePidPath(),
      'engine-path': getEnginePath(platform, arch),
      'aria2-bin-path': getAria2BinPath(platform, arch),
      'aria2-conf-path': getAria2ConfPath(platform, arch)
    }
  }

  get (key?: keyof MotrixContext): any {
    if (!key) return this.context
    return this.context[key]
  }
}
