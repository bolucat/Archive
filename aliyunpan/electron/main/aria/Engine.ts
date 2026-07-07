import { spawn, ChildProcess } from 'node:child_process'
import { existsSync, writeFile, unlink } from 'node:fs'
import is from 'electron-is'
import {
  getEnginePidPath, getAria2BinPath, getAria2ConfPath,
  getSessionPath, transformConfig
} from './utils'
import logger from './Logger'

export interface EngineOptions {
  systemConfig: Record<string, any>
  userConfig: Record<string, any>
}

export default class Engine {
  systemConfig: Record<string, any>
  userConfig: Record<string, any>
  instance: ChildProcess | null = null

  constructor (options: EngineOptions) {
    this.systemConfig = options.systemConfig || {}
    this.userConfig = options.userConfig || {}
  }

  start (): void {
    if (this.instance) return
    const binPath = this.getEngineBinPath()
    const args = this.getStartArgs()
    logger.info(`[motrix] Engine.start binPath=${binPath}`)
    this.instance = spawn(binPath, args, {
      windowsHide: false,
      stdio: is.dev() ? 'pipe' : 'ignore'
    })
    if (!this.instance || !this.instance.pid) {
      logger.error('[motrix] Engine spawn failed')
      return
    }
    const pid = String(this.instance.pid)
    this.writePidFile(getEnginePidPath(), pid)
    this.instance.once('close', () => {
      unlink(getEnginePidPath(), (err) => {
        if (err) logger.warn('[motrix] unlink engine.pid failed: ' + err.message)
      })
    })
  }

  stop (): void {
    if (!this.instance) return
    try { this.instance.kill() } catch (e: any) {
      logger.warn('[motrix] Engine.stop: ' + e.message)
    }
    this.instance = null
  }

  restart (): void { this.stop(); this.start() }

  writePidFile (pidPath: string, pid: string): void {
    writeFile(pidPath, pid, (err) => {
      if (err) logger.warn('[motrix] write engine.pid failed: ' + err.message)
    })
  }

  getEngineBinPath (): string {
    const binPath = getAria2BinPath(process.platform, process.arch)
    if (!existsSync(binPath)) throw new Error(`aria2c binary missing at ${binPath}`)
    return binPath
  }

  getStartArgs (): string[] {
    const confPath = getAria2ConfPath(process.platform, process.arch)
    const sessionPath = getSessionPath()
    const sessionExist = existsSync(sessionPath)
    const result: string[] = [
      `--conf-path=${confPath}`,
      `--save-session=${sessionPath}`
    ]
    if (sessionExist) result.push(`--input-file=${sessionPath}`)

    const extraConfig: Record<string, any> = { ...this.systemConfig }
    const keepSeeding = this.userConfig['keep-seeding']
    const seedRatio = this.systemConfig['seed-ratio']
    if (keepSeeding || seedRatio === 0) {
      extraConfig['seed-ratio'] = 0
      delete extraConfig['seed-time']
    }
    result.push(...transformConfig(extraConfig))
    return result
  }

  isRunning (pid: number): boolean {
    try { process.kill(pid, 0); return true }
    catch (err: any) { return err.code === 'EPERM' }
  }
}
