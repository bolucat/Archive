import { EventEmitter } from 'node:events'
import { describe, expect, it, vi } from 'vitest'
import { createAutoUpdateController } from '../autoUpdate'

class FakeUpdater extends EventEmitter {
  autoDownload = true
  autoInstallOnAppQuit = true
  allowPrerelease = false
  checkForUpdates = vi.fn()
  downloadUpdate = vi.fn().mockResolvedValue(undefined)
  quitAndInstall = vi.fn()
  setFeedURL = vi.fn()
}

describe('createAutoUpdateController', () => {
  it('downloads an available update silently in the background', async () => {
    const updater = new FakeUpdater()
    const dialog = {
      showMessageBox: vi.fn().mockResolvedValue({ response: 0 })
    }

    createAutoUpdateController({
      updater,
      dialog,
      logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
      currentVersion: '4.0.11-beta',
      isPackaged: true,
    })

    updater.emit('update-available', { version: '4.0.12-beta', releaseNotes: '修复若干问题' })
    await Promise.resolve()

    expect(dialog.showMessageBox).not.toHaveBeenCalled()
    expect(updater.downloadUpdate).toHaveBeenCalledTimes(1)
    expect(updater.allowPrerelease).toBe(true)
    expect(updater.autoDownload).toBe(false)
    expect(updater.autoInstallOnAppQuit).toBe(true)
    expect(updater.setFeedURL).toHaveBeenCalledWith('https://gh-proxy.com/https://github.com/gaozhangmin/boxplayer/releases/latest/download/')
  })

  it('prompts to restart after an update is downloaded', async () => {
    const updater = new FakeUpdater()
    const dialog = {
      showMessageBox: vi.fn().mockResolvedValue({ response: 0 })
    }

    createAutoUpdateController({
      updater,
      dialog,
      logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
      currentVersion: '4.0.11-beta',
      isPackaged: true,
    })

    updater.emit('update-downloaded', { version: '4.0.12-beta' })
    await Promise.resolve()

    expect(dialog.showMessageBox).toHaveBeenCalledWith(expect.objectContaining({
      type: 'info',
      title: '更新已下载',
      message: '新版本 4.0.12-beta 已在后台下载完成',
      detail: '重启 App 即可完成更新安装。',
      buttons: ['重启安装', '稍后']
    }))
    expect(updater.quitAndInstall).toHaveBeenCalledWith(false, true)
  })
})
