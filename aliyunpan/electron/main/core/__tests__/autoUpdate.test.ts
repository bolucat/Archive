import { EventEmitter } from 'node:events'
import { describe, expect, it, vi } from 'vitest'
import { createAutoUpdateController, readUpdateProxyPreferences } from '../autoUpdate'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'

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
    expect(updater.setFeedURL).toHaveBeenCalledWith('https://github.com/gaozhangmin/boxplayer/releases/latest/download/')
  })

  it('publishes background download progress to the renderer state', () => {
    const updater = new FakeUpdater()
    const onStateChange = vi.fn()
    const controller = createAutoUpdateController({
      updater,
      dialog: { showMessageBox: vi.fn() },
      logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
      currentVersion: '4.0.11',
      isPackaged: true,
      onStateChange
    })

    updater.emit('update-available', { version: '4.0.12' })
    updater.emit('download-progress', { percent: 42.6, transferred: 426, total: 1000, bytesPerSecond: 128 })

    expect(controller.getState()).toEqual({
      status: 'downloading',
      version: '4.0.12',
      percent: 42.6,
      transferred: 426,
      total: 1000,
      bytesPerSecond: 128
    })
    expect(onStateChange).toHaveBeenLastCalledWith(controller.getState())
  })

  it('shares one completed startup check between automatic and renderer requests', async () => {
    const updater = new FakeUpdater()
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-not-available', { version: '4.0.11' })
    })
    const controller = createAutoUpdateController({
      updater,
      dialog: { showMessageBox: vi.fn() },
      logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
      currentVersion: '4.0.11',
      isPackaged: true
    })

    expect(await controller.checkNow()).toEqual({ status: 'up-to-date', version: '4.0.11' })
    expect(await controller.checkNow()).toEqual({ status: 'up-to-date', version: '4.0.11' })
    expect(updater.checkForUpdates).toHaveBeenCalledTimes(1)
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

  it('installs a downloaded update when requested from the renderer', () => {
    const updater = new FakeUpdater()
    const controller = createAutoUpdateController({ updater, dialog: { showMessageBox: vi.fn().mockResolvedValue({ response: 1 }) }, logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() }, currentVersion: '4.0.11', isPackaged: true })

    expect(controller.installNow()).toBe(false)
    updater.emit('update-downloaded', { version: '4.0.12' })
    expect(controller.installNow()).toBe(true)
    expect(updater.quitAndInstall).toHaveBeenCalledWith(false, true)
  })

  it('falls back to GitHub when the configured proxy cannot check for updates', async () => {
    const updater = new FakeUpdater()
    const dialog = { showMessageBox: vi.fn() }
    createAutoUpdateController({ updater, dialog, logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() }, currentVersion: '4.0.11', isPackaged: true, updateProxy: { enabled: true, url: 'https://gh-proxy.com' } })

    updater.emit('error', new Error('proxy unavailable'))
    await Promise.resolve()

    expect(updater.setFeedURL).toHaveBeenLastCalledWith('https://github.com/gaozhangmin/boxplayer/releases/latest/download/')
    expect(updater.checkForUpdates).toHaveBeenCalledTimes(1)
  })

  it('uses the configured proxy for both checking and background downloading', async () => {
    const updater = new FakeUpdater()
    const controller = createAutoUpdateController({
      updater,
      dialog: { showMessageBox: vi.fn() },
      logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
      currentVersion: '4.0.11',
      isPackaged: true,
      updateProxy: { enabled: true, url: 'https://update-proxy.example/' }
    })

    await controller.checkNow()
    updater.emit('update-available', { version: '4.0.12' })
    await Promise.resolve()

    expect(updater.setFeedURL).toHaveBeenLastCalledWith('https://update-proxy.example/https://github.com/gaozhangmin/boxplayer/releases/latest/download/')
    expect(updater.downloadUpdate).toHaveBeenCalledTimes(1)
  })

  it('uses GitHub directly when the proxy is disabled or invalid', () => {
    const disabledUpdater = new FakeUpdater()
    const invalidUpdater = new FakeUpdater()
    const createController = (updater: FakeUpdater, updateProxy: { enabled: boolean; url: string }) => createAutoUpdateController({ updater, dialog: { showMessageBox: vi.fn() }, logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() }, currentVersion: '4.0.11', isPackaged: true, updateProxy })

    createController(disabledUpdater, { enabled: false, url: '' })
    createController(invalidUpdater, { enabled: true, url: 'ftp://invalid.example' })

    const defaultFeed = 'https://github.com/gaozhangmin/boxplayer/releases/latest/download/'
    expect(disabledUpdater.setFeedURL).toHaveBeenCalledWith(defaultFeed)
    expect(invalidUpdater.setFeedURL).toHaveBeenCalledWith(defaultFeed)
  })

  it('falls back to GitHub and retries after a proxied download fails', async () => {
    const updater = new FakeUpdater()
    updater.downloadUpdate.mockRejectedValueOnce(new Error('proxy download unavailable')).mockResolvedValueOnce(undefined)
    createAutoUpdateController({
      updater,
      dialog: { showMessageBox: vi.fn() },
      logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
      currentVersion: '4.0.11',
      isPackaged: true,
      updateProxy: { enabled: true, url: 'https://gh-proxy.com' }
    })

    updater.emit('update-available', { version: '4.0.12' })
    await new Promise(resolve => setTimeout(resolve, 0))

    expect(updater.setFeedURL).toHaveBeenLastCalledWith('https://github.com/gaozhangmin/boxplayer/releases/latest/download/')
    expect(updater.checkForUpdates).toHaveBeenCalledTimes(1)

    updater.emit('update-available', { version: '4.0.12' })
    await Promise.resolve()
    expect(updater.downloadUpdate).toHaveBeenCalledTimes(2)
  })

  it('reads the update proxy switch and URL from setting.config', () => {
    const userDataPath = mkdtempSync(join(tmpdir(), 'boxplayer-update-'))
    writeFileSync(join(userDataPath, 'setting.config'), JSON.stringify({
      uiUpdateProxyEnable: true,
      uiUpdateProxyUrl: 'https://gh-proxy.com/',
      uiLaunchAutoCheckUpdate: true
    }))

    expect(readUpdateProxyPreferences(userDataPath)).toEqual({
      enabled: true,
      url: 'https://gh-proxy.com/',
      autoCheckOnLaunch: true
    })
  })

  it('does not check or download updates on launch when startup checking is disabled', async () => {
    vi.useFakeTimers()
    const updater = new FakeUpdater()
    createAutoUpdateController({
      updater,
      dialog: { showMessageBox: vi.fn() },
      logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
      currentVersion: '4.0.11',
      isPackaged: true,
      updateProxy: { enabled: false, url: '', autoCheckOnLaunch: false }
    })

    await vi.advanceTimersByTimeAsync(2_000)
    expect(updater.checkForUpdates).not.toHaveBeenCalled()
    expect(updater.downloadUpdate).not.toHaveBeenCalled()
    vi.useRealTimers()
  })

  it('still allows a manual update check when startup checking is disabled', async () => {
    const updater = new FakeUpdater()
    updater.checkForUpdates.mockImplementation(async () => {
      updater.emit('update-not-available', { version: '4.0.11' })
    })
    const controller = createAutoUpdateController({
      updater,
      dialog: { showMessageBox: vi.fn() },
      logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
      currentVersion: '4.0.11',
      isPackaged: true,
      updateProxy: { enabled: false, url: '', autoCheckOnLaunch: false }
    })

    expect(await controller.checkNow(true)).toEqual({ status: 'up-to-date', version: '4.0.11' })
    expect(updater.checkForUpdates).toHaveBeenCalledTimes(1)
  })
})
