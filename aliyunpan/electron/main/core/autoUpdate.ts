import { app, dialog } from 'electron'
import { autoUpdater } from 'electron-updater'
import type { UpdateInfo } from 'electron-updater'
import is from 'electron-is'

const UPDATE_CHECK_DELAY_MS = 8000
const GITHUB_PROXY_UPDATE_FEED_URL = 'https://gh-proxy.com/https://github.com/gaozhangmin/boxplayer/releases/latest/download/'

type AutoUpdateLogger = Pick<typeof console, 'info' | 'warn'>
type AutoUpdateDialog = Pick<typeof dialog, 'showMessageBox'>
type AutoUpdateControllerOptions = {
  updater: typeof autoUpdater
  dialog: AutoUpdateDialog
  logger: AutoUpdateLogger
  currentVersion: string
  isPackaged: boolean
  isMas?: boolean
}

export function createAutoUpdateController(options: AutoUpdateControllerOptions) {
  const { updater, dialog, logger, currentVersion, isPackaged, isMas = false } = options

  if (isMas) return

  if (!isPackaged) return

  updater.autoDownload = false
  updater.autoInstallOnAppQuit = true
  updater.allowPrerelease = currentVersion.includes('-')
  updater.setFeedURL(GITHUB_PROXY_UPDATE_FEED_URL)

  let hasStartedDownload = false
  let hasPromptedRestart = false

  updater.on('update-available', (info: UpdateInfo) => {
    if (hasStartedDownload) return
    hasStartedDownload = true
    logger.info('[auto-update] update available, downloading in background', info.version)
    updater.downloadUpdate().catch((err: unknown) => {
      logger.warn('[auto-update] download failed', err)
    })
  })

  updater.on('update-downloaded', (info: UpdateInfo) => {
    if (hasPromptedRestart) return
    hasPromptedRestart = true
    dialog.showMessageBox({
      type: 'info',
      title: '更新已下载',
      message: `新版本 ${info.version} 已在后台下载完成`,
      detail: '重启 App 即可完成更新安装。',
      buttons: ['重启安装', '稍后'],
      defaultId: 0,
      cancelId: 1
    }).then(({ response }) => {
      if (response === 0) updater.quitAndInstall(false, true)
    }).catch((err: unknown) => {
      logger.warn('[auto-update] restart prompt failed', err)
    })
  })

  updater.on('error', (err: unknown) => {
    logger.warn('[auto-update] updater error', err)
  })

  setTimeout(() => {
    updater.checkForUpdates().catch((err: unknown) => {
      logger.warn('[auto-update] check failed', err)
    })
  }, UPDATE_CHECK_DELAY_MS)
}

export function registerAutoUpdate() {
  createAutoUpdateController({
    updater: autoUpdater,
    dialog,
    logger: console,
    currentVersion: app.getVersion(),
    isPackaged: app.isPackaged,
    isMas: is.mas()
  })
}
