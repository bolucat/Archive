import { describe, expect, it, vi, beforeEach } from 'vitest'
import path from 'path'
import os from 'os'
import fs from 'fs'
import {
  NGOSANG_TRACKERS_BEST_CDN_URL,
  NGOSANG_TRACKERS_BEST_IP_CDN_URL,
  NGOSANG_TRACKERS_BEST_URL
} from '@shared/constants'

vi.mock('electron', () => ({
  app: {
    getPath: vi.fn(() => path.join(os.tmpdir(), 'motrix-test-config')),
    getAppPath: vi.fn(() => os.tmpdir()),
    getLocale: vi.fn(() => 'zh-CN'),
    getLoginItemSettings: vi.fn(() => ({ openAtLogin: false }))
  }
}))

const tmpDir = path.join(os.tmpdir(), 'motrix-test-config')

beforeEach(() => {
  fs.mkdirSync(tmpDir, { recursive: true })
  try { fs.unlinkSync(path.join(tmpDir, 'system.json')) } catch {}
  try { fs.unlinkSync(path.join(tmpDir, 'user.json')) } catch {}
})

describe('ConfigManager', () => {
  it('initializes system config with defaults', async () => {
    const { default: ConfigManager } = await import('../ConfigManager')
    const cm = new ConfigManager()
    expect(cm.getSystemConfig('rpc-listen-port')).toBe(16800)
    expect(cm.getSystemConfig('rpc-secret')).toBe('S4znWTaZYQi3cpRNb')
  })

  it('initializes user config with defaults', async () => {
    const { default: ConfigManager } = await import('../ConfigManager')
    const cm = new ConfigManager()
    expect(cm.getUserConfig('auto-sync-tracker')).toBe(true)
    expect(cm.getUserConfig('enable-upnp')).toBe(true)
    expect(cm.getUserConfig('tracker-source')).toEqual([
      NGOSANG_TRACKERS_BEST_IP_CDN_URL,
      NGOSANG_TRACKERS_BEST_CDN_URL
    ])
    expect(typeof cm.getUserConfig('locale')).toBe('string')
  })

  it('migrates the old non-Motrix BT defaults', async () => {
    fs.writeFileSync(path.join(tmpDir, 'user.json'), JSON.stringify({
      'auto-sync-tracker': false,
      'enable-upnp': false,
      'tracker-source': [NGOSANG_TRACKERS_BEST_URL],
      'last-sync-tracker-time': 123
    }))
    const { default: ConfigManager } = await import('../ConfigManager')
    const cm = new ConfigManager()
    expect(cm.getUserConfig('auto-sync-tracker')).toBe(true)
    expect(cm.getUserConfig('enable-upnp')).toBe(true)
    expect(cm.getUserConfig('tracker-source')).toEqual([
      NGOSANG_TRACKERS_BEST_IP_CDN_URL,
      NGOSANG_TRACKERS_BEST_CDN_URL
    ])
    expect(cm.getUserConfig('last-sync-tracker-time')).toBe(0)
  })

  it('setSystemConfig and getSystemConfig round-trips', async () => {
    const { default: ConfigManager } = await import('../ConfigManager')
    const cm = new ConfigManager()
    cm.setSystemConfig('split', 8)
    expect(cm.getSystemConfig('split')).toBe(8)
  })

  it('setUserConfig and getUserConfig round-trips', async () => {
    const { default: ConfigManager } = await import('../ConfigManager')
    const cm = new ConfigManager()
    cm.setUserConfig('keep-seeding', true)
    expect(cm.getUserConfig('keep-seeding')).toBe(true)
  })
})
