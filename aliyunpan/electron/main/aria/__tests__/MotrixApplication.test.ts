import { describe, expect, it, vi } from 'vitest'

vi.mock('electron', () => ({
  app: {
    getPath: vi.fn(() => '/tmp/motrix-app-test'),
    getAppPath: vi.fn(() => '/tmp'),
    getLocale: vi.fn(() => 'zh-CN'),
    getLoginItemSettings: vi.fn(() => ({ openAtLogin: false }))
  },
  ipcMain: { handle: vi.fn(), on: vi.fn(), off: vi.fn() },
  BrowserWindow: { getAllWindows: vi.fn(() => []) },
  dialog: { showMessageBox: vi.fn() }
}))

vi.mock('child_process', () => ({
  spawn: vi.fn(() => ({
    pid: 999,
    on: vi.fn(),
    stdout: { on: vi.fn() },
    stderr: { on: vi.fn() },
    kill: vi.fn(() => true)
  }))
}))

vi.mock('aria2-lib', () => ({
  default: vi.fn().mockImplementation(() => ({
    open: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
    call: vi.fn().mockResolvedValue({}),
    on: vi.fn(),
    setMaxListeners: vi.fn()
  }))
}))

vi.mock('@motrix/nat-api', () => ({
  default: vi.fn().mockImplementation(() => ({
    map: vi.fn().mockResolvedValue(undefined),
    unmap: vi.fn().mockResolvedValue(undefined),
    destroy: vi.fn().mockResolvedValue(undefined)
  }))
}))

describe('MotrixApplication', () => {
  it('can be instantiated', async () => {
    const { default: MotrixApplication } = await import('../MotrixApplication')
    const app = new MotrixApplication()
    expect(app).toBeDefined()
  })

  it('initialized flag starts false', async () => {
    const { default: MotrixApplication } = await import('../MotrixApplication')
    const app = new MotrixApplication() as any
    expect(app.initialized).toBe(false)
  })

  it('quit() resolves without throwing when not started', async () => {
    const { default: MotrixApplication } = await import('../MotrixApplication')
    const app = new MotrixApplication()
    await expect(app.quit()).resolves.not.toThrow()
  })
})
