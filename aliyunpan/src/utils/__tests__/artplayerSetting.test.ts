import { describe, expect, it } from 'vitest'
import { updateSettingPreservingActivePanel } from '../artplayerSetting'

describe('updateSettingPreservingActivePanel', () => {
  it('keeps the active subtitle settings panel open after its selector is rebuilt', () => {
    const previousSelector = [{ html: '旧字幕' }]
    const subtitleSetting = { name: 'Subtitle', selector: previousSelector }
    const refreshedSetting = { name: 'Subtitle', selector: [{ html: '内嵌字幕' }, { html: '子目录字幕.ass' }] }
    const rendered: unknown[] = []
    const settings = {
      active: previousSelector,
      find: () => subtitleSetting,
      update: () => refreshedSetting,
      render: (selector: unknown[]) => rendered.push(selector)
    }

    updateSettingPreservingActivePanel(settings, refreshedSetting)

    expect(rendered).toEqual([refreshedSetting.selector])
  })

  it('does not open a nested panel when it was not active before the refresh', () => {
    const subtitleSetting = { name: 'Subtitle', selector: [{ html: '旧字幕' }] }
    const refreshedSetting = { name: 'Subtitle', selector: [{ html: '新字幕' }] }
    const rendered: unknown[] = []
    const settings = {
      active: [],
      find: () => subtitleSetting,
      update: () => refreshedSetting,
      render: (selector: unknown[]) => rendered.push(selector)
    }

    updateSettingPreservingActivePanel(settings, refreshedSetting)

    expect(rendered).toEqual([])
  })
})
