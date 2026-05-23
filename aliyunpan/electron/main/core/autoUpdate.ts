import { app } from 'electron'
import { autoUpdater } from 'electron-updater'
import type { UpdateInfo } from 'electron-updater'
import is from 'electron-is'
import { AppWindow } from './window'

const UPDATE_CHECK_DELAY_MS = 8000

export function registerAutoUpdate() {
  if (is.mas()) return

  if (!app.isPackaged) return

  autoUpdater.autoDownload = false
  autoUpdater.autoInstallOnAppQuit = false
  autoUpdater.allowPrerelease = app.getVersion().includes('-')

  let hasPrompted = false

  autoUpdater.on('update-available', (_info: UpdateInfo) => {
    if (hasPrompted) return
    hasPrompted = true
    const mainWindow = AppWindow.mainWindow
    if (!mainWindow || mainWindow.isDestroyed()) return
    mainWindow.webContents.send('showUpdateModal')
  })

  autoUpdater.on('error', (err: unknown) => {
    console.warn('[auto-update] updater error', err)
  })

  setTimeout(() => {
    autoUpdater.checkForUpdates().catch((err: unknown) => {
      console.warn('[auto-update] check failed', err)
    })
  }, UPDATE_CHECK_DELAY_MS)
}
