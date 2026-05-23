import { EventEmitter } from 'node:events'
import { describe, expect, it, vi } from 'vitest'
import { createAutoUpdateController } from '../autoUpdate'

class FakeUpdater extends EventEmitter {
  autoDownload = true
  autoInstallOnAppQuit = true
  allowPrerelease = false
  checkForUpdates = vi.fn()
  downloadUpdate = vi.fn()
  quitAndInstall = vi.fn()
}

describe('createAutoUpdateController', () => {
  it('prompts before downloading an available update', async () => {
    const updater = new FakeUpdater()
    const dialog = {
      showMessageBox: vi.fn().mockResolvedValue({ response: 0 }),
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

    expect(dialog.showMessageBox).toHaveBeenCalledWith(expect.objectContaining({
      type: 'info',
      title: '发现新版本',
      buttons: ['立即下载', '稍后'],
    }))
    expect(updater.downloadUpdate).toHaveBeenCalledTimes(1)
    expect(updater.allowPrerelease).toBe(true)
    expect(updater.autoDownload).toBe(false)
  })

  it('prompts to restart after an update is downloaded', async () => {
    const updater = new FakeUpdater()
    const dialog = {
      showMessageBox: vi.fn().mockResolvedValue({ response: 0 }),
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
      buttons: ['重启安装', '稍后'],
    }))
    expect(updater.quitAndInstall).toHaveBeenCalledWith(false, true)
  })
})
