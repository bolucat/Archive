import { describe, expect, it, vi } from 'vitest'

vi.mock('@motrix/nat-api', () => ({
  default: vi.fn().mockImplementation(() => ({
    map: vi.fn((port: number, cb: Function) => cb(null)),
    unmap: vi.fn((port: number, cb: Function) => cb(null)),
    destroy: vi.fn((cb: Function) => cb())
  }))
}))

describe('UPnPManager', () => {
  it('can be instantiated', async () => {
    const { default: UPnPManager } = await import('../UPnPManager')
    const mgr = new UPnPManager()
    expect(mgr).toBeDefined()
  })

  it('unmap() of unmapped port resolves without error', async () => {
    const { default: UPnPManager } = await import('../UPnPManager')
    const mgr = new UPnPManager()
    await expect(mgr.unmap(21301)).resolves.not.toThrow()
  })

  it('closeClient() resolves without error', async () => {
    const { default: UPnPManager } = await import('../UPnPManager')
    const mgr = new UPnPManager()
    await expect(mgr.closeClient()).resolves.not.toThrow()
  })
})
