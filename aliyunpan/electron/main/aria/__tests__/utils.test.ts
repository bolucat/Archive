import { describe, expect, it, vi } from 'vitest'

vi.mock('electron', () => ({
  app: {
    getPath: vi.fn(() => '/tmp/motrix-utils-test'),
    getAppPath: vi.fn(() => '/Users/test/aliyunpan')
  }
}))

vi.mock('electron-is', () => ({
  default: {
    dev: () => true
  }
}))

describe('aria utils', () => {
  it('resolves dev engine binaries from the static engine directory', async () => {
    const { getAria2BinPath, getAria2ConfPath } = await import('../utils')

    expect(getAria2BinPath('darwin', 'arm64')).toBe('/Users/test/aliyunpan/static/engine/darwin/arm64/aria2c')
    expect(getAria2ConfPath('darwin', 'arm64')).toBe('/Users/test/aliyunpan/static/engine/darwin/arm64/aria2.conf')
  })
})
