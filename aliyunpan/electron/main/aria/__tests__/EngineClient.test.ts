import { describe, expect, it, vi } from 'vitest'

vi.mock('aria2-lib', () => ({
  default: vi.fn().mockImplementation(() => ({
    open: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
    call: vi.fn().mockResolvedValue({}),
    on: vi.fn(),
    setMaxListeners: vi.fn()
  }))
}))

vi.mock('electron', () => ({
  app: {
    getPath: vi.fn(() => '/tmp'),
    getAppPath: vi.fn(() => '/tmp'),
    getLocale: vi.fn(() => 'zh-CN'),
    getLoginItemSettings: vi.fn(() => ({ openAtLogin: false }))
  }
}))

describe('EngineClient', () => {
  it('can be instantiated', async () => {
    const { default: EngineClient } = await import('../EngineClient')
    const client = new EngineClient({ port: 16800, secret: 'test' })
    expect(client).toBeDefined()
  })

  it('init() calls connect and does not throw', async () => {
    const { default: EngineClient } = await import('../EngineClient')
    const client = new EngineClient({ port: 16800, secret: 'test' })
    expect(() => client.init()).not.toThrow()
  })
})
