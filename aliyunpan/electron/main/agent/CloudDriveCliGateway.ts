import { app } from 'electron'
import path from 'path'
import { pathToFileURL } from 'url'

export interface CloudDriveCliResult {
  exitCode: number
  stdout: string
  stderr: string
}

export async function runBundledCloudDriveCli(args: string[]): Promise<CloudDriveCliResult> {
  const modulePath = path.join(app.getAppPath(), 'clouddrive-cli', 'core', 'commands.mjs')
  const mod = await import(pathToFileURL(modulePath).href) as { runBoxPlayerCli: (argv: string[]) => Promise<CloudDriveCliResult> }
  return mod.runBoxPlayerCli(args)
}

export async function runBundledCloudDriveCliJson<T>(args: string[]): Promise<T> {
  const result = await runBundledCloudDriveCli([...args, '--json'])
  let body: any
  try {
    body = result.stdout ? JSON.parse(result.stdout) : undefined
  } catch {
    throw new Error(result.stderr || result.stdout || '网盘 CLI 返回了无效结果')
  }
  if (result.exitCode !== 0) throw new Error(body?.error?.message || body?.error || result.stderr || '网盘 CLI 执行失败')
  return body as T
}
