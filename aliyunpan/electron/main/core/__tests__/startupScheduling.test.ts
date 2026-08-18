import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const root = resolve(__dirname, '../../../..')

describe('startup scheduling', () => {
  it('does not wake media acquisition immediately from the main process', () => {
    const source = readFileSync(resolve(root, 'electron/main/mediaAcquisition/MediaAcquisitionWakeScheduler.ts'), 'utf8')

    expect(source).toContain('const WAKE_INTERVAL_MS = 60_000')
    expect(source).not.toMatch(/\n\s*wake\(\)\n\s*timer = setInterval/)
  })

  it('creates transfer workers only when work is queued', () => {
    const windowSource = readFileSync(resolve(root, 'electron/main/core/window.ts'), 'utf8')
    const mainWindowStart = windowSource.indexOf('export function createMainWindow')
    const mainWindowEnd = windowSource.indexOf('function createUpload', mainWindowStart)
    const createMainWindowSource = windowSource.slice(mainWindowStart, mainWindowEnd)

    expect(createMainWindowSource).not.toContain('createUpload()')
    expect(createMainWindowSource).not.toContain('createDownload()')
    expect(windowSource).toContain("ipcMain.on('EnsureTransferWorker'")
  })

  it('starts account recovery before the optional playback proxy', () => {
    const source = readFileSync(resolve(root, 'src/layout/PageMain.ts'), 'utf8')
    const accountLoad = source.indexOf('await UserDAL.aLoadFromDB()')
    const proxyStartup = source.indexOf("label: 'CreateProxyServer'")

    expect(accountLoad).toBeGreaterThan(-1)
    expect(proxyStartup).toBeGreaterThan(accountLoad)
  })
})
