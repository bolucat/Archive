import { describe, expect, it } from 'vitest'
import { BOXPLAYER_CAPABILITIES, buildBoxPlayerCapabilityKnowledge, getBoxPlayerCapability } from '../../services/agent/boxplayerCapabilities'

describe('BoxPlayer Agent capability registry', () => {
  it('documents every top-level workspace module with examples and limitations', () => {
    expect(BOXPLAYER_CAPABILITIES.map(capability => capability.id)).toEqual(expect.arrayContaining([
      'cloud-files', 'sharing', 'transfers', 'media-discovery', 'media-acquisition', 'upload', 'playback', 'media-server', 'books', 'settings'
    ]))
    for (const capability of BOXPLAYER_CAPABILITIES) {
      expect(capability.examples.length).toBeGreaterThan(0)
      expect(capability.limitations.length).toBeGreaterThan(0)
      expect(capability.tools.length).toBeGreaterThan(0)
    }
  })

  it('keeps navigation-only capabilities explicitly non-executable', () => {
    expect(getBoxPlayerCapability('settings')).toMatchObject({ status: 'guided', mode: 'navigate', tab: 'setting' })
    expect(getBoxPlayerCapability('cloud-files')).toMatchObject({ status: 'available', mode: 'write' })
    expect(getBoxPlayerCapability('media-acquisition')).toMatchObject({ status: 'available', mode: 'write', tab: 'ai' })
  })

  it('builds prompt-ready knowledge without omitting safety limits', () => {
    const knowledge = buildBoxPlayerCapabilityKnowledge()
    expect(knowledge).toContain('创建新分享链接尚未开放给 Agent')
    expect(knowledge).toContain('Agent 暂不能修改设置')
    expect(knowledge).toContain('分享导入失败时仅在目标网盘支持离线下载时继续 fallback')
  })
})
