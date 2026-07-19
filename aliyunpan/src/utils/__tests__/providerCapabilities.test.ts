import { describe, expect, it } from 'vitest'
import { formatProviderCapabilities, getProviderCapabilities, normalizeProviderPlatform, PROVIDER_CAPABILITIES } from '../../services/agent/providerCapabilities'
import { getMediaAcquisitionCapability } from '../../services/mediaAcquisition/capabilities'

describe('provider capability manifest', () => {
  it('normalizes persisted provider aliases before resolving a manifest', () => {
    expect(normalizeProviderPlatform('139')).toBe('cloud139')
    expect(normalizeProviderPlatform('drive115')).toBe('115')
    expect(getProviderCapabilities('drive115')).toMatchObject({ platform: '115', capabilities: { recycleBin: true } })
  })

  it('keeps unknown providers restricted instead of assuming write access', () => {
    const unknown = getProviderCapabilities('future-drive')
    expect(unknown.capabilities.list).toBe(true)
    expect(unknown.capabilities.search).toBe(false)
    expect(unknown.capabilities.move).toBe(false)
    expect(unknown.capabilities.share).toBe(false)
  })

  it('reports provider-specific limitations in prompt-ready runtime knowledge', () => {
    const text = formatProviderCapabilities([getProviderCapabilities('guangya'), getProviderCapabilities('webdav')])
    expect(text).toContain('光鸭云')
    expect(text).toContain('WebDAV')
    expect(text).toContain('写操作需在网盘界面完成')
  })

  it('derives UI and media acquisition operations from the provider manifest', () => {
    const pikpak = getProviderCapabilities('pikpak')
    expect(pikpak.operations['share.import']).toBe(true)
    expect(pikpak.operations['offline.magnet']).toBe(true)
    expect(getMediaAcquisitionCapability('pikpak')).toMatchObject({ shareImport: true, magnetOfflineDownload: true, externalUrlOfflineDownload: true })

    const box = getProviderCapabilities('box')
    expect(box.operations['upload.local']).toBe(false)
    expect(box.operations['upload.memory']).toBe(true)
  })

  it('advertises agent tools from operation-level capabilities', () => {
    expect(getProviderCapabilities('cloud139').agentTools).toContain('deleteFiles')
    expect(getProviderCapabilities('webdav').agentTools).not.toContain('deleteFiles')
  })

  it('requires every adapter-owned manifest entry to declare verification evidence', () => {
    for (const manifest of Object.values(PROVIDER_CAPABILITIES)) {
      for (const [capability, enabled] of Object.entries(manifest.capabilities)) {
        const evidence = manifest.evidence[capability as keyof typeof manifest.evidence]
        expect(evidence).toBeDefined()
        expect(enabled ? ['tested', 'implemented'] : ['unsupported', 'unknown']).toContain(evidence.status)
      }
    }
  })
})
