import { describe, expect, it } from 'vitest'
import { mkdirSync, mkdtempSync, writeFileSync } from 'fs'
import path from 'path'
import { tmpdir } from 'os'
import { getEmbeddedMpvCapability } from '../../mpv/embeddedMpvCapability'
import { DisabledEmbeddedMpvBridge } from '../../mpv/embeddedMpvBridge'
import { getEmbeddedMpvNativeResourceStatus, loadEmbeddedMpvNativeAddon } from '../../mpv/embeddedMpvNativeAddon'

describe('embedded MPV capability', () => {
  it('enables macOS embedded MPV when native addon and resources are complete', () => {
    const capability = getEmbeddedMpvCapability({
      platform: 'darwin',
      electronVersion: '21.4.4',
      nativeAddonAvailable: true,
      nativeResourcesComplete: true
    })

    expect(capability.enabled).toBe(true)
    expect(capability.status).toBe('available')
  })

  it('keeps non-macOS platforms on the external mpv route', () => {
    const capability = getEmbeddedMpvCapability({
      platform: 'linux',
      electronVersion: '40.0.0',
      featureFlag: '1',
      nativeAddonAvailable: true
    })

    expect(capability.enabled).toBe(false)
    expect(capability.status).toBe('unsupported-platform')
  })

  it('requires native addon and complete resources on macOS', () => {
    expect(getEmbeddedMpvCapability({ platform: 'darwin', electronVersion: '40.0.0' }).status).toBe('disabled')
    expect(getEmbeddedMpvCapability({ platform: 'darwin', electronVersion: '40.0.0', nativeAddonAvailable: true }).status).toBe('disabled')
    expect(getEmbeddedMpvCapability({ platform: 'darwin', electronVersion: '40.0.0', nativeAddonAvailable: true, nativeResourcesComplete: true }).status).toBe('available')
  })

  it('requires a complete macOS native resource bundle before enabling embedded MPV', () => {
    const dir = mkdtempSync(path.join(tmpdir(), 'boxplayer-mpv-resources-'))
    const addonPath = path.join(dir, 'boxplayer-mpv-texture.node')
    writeFileSync(addonPath, '')

    const incomplete = getEmbeddedMpvNativeResourceStatus([addonPath])
    expect(incomplete.complete).toBe(false)
    expect(incomplete.missing).toContain(path.join(dir, 'libmpv.dylib'))
    expect(incomplete.missing).toContain(path.join(dir, 'mpv-bundle-manifest.json'))

    writeFileSync(path.join(dir, 'libmpv.dylib'), '')
    writeFileSync(path.join(dir, 'mpv-bundle-manifest.json'), JSON.stringify({ files: [{ name: 'boxplayer-mpv-texture.node' }] }))

    const missingManifestEntry = getEmbeddedMpvNativeResourceStatus([addonPath])
    expect(missingManifestEntry.complete).toBe(false)
    expect(missingManifestEntry.missing).toEqual(['mpv-bundle-manifest.json:libmpv.dylib'])
  })

  it('accepts a complete macOS native resource bundle manifest', () => {
    const dir = mkdtempSync(path.join(tmpdir(), 'boxplayer-mpv-resources-'))
    const addonPath = path.join(dir, 'boxplayer-mpv-texture.node')
    writeFileSync(addonPath, '')
    writeFileSync(path.join(dir, 'libmpv.dylib'), '')
    writeFileSync(
      path.join(dir, 'mpv-bundle-manifest.json'),
      JSON.stringify({
        files: [
          { name: 'boxplayer-mpv-texture.node', sha256: 'node-sha' },
          { name: 'libmpv.dylib', sha256: 'libmpv-sha' }
        ]
      })
    )

    const result = getEmbeddedMpvNativeResourceStatus([addonPath])
    expect(result.complete).toBe(true)
    expect(result.directory).toBe(dir)
    expect(result.missing).toEqual([])
  })

  it('disabled bridge refuses to load and returns capability context', async () => {
    const bridge = new DisabledEmbeddedMpvBridge()
    const result = await bridge.load({ url: 'https://example.test/video.mkv' })

    expect(result.ok).toBe(false)
    expect(result.capability.enabled).toBe(false)
    expect(result.error).toBeTruthy()
  })

  it('disabled bridge refuses control and status calls with capability context', async () => {
    const bridge = new DisabledEmbeddedMpvBridge()

    const controlResult = await bridge.control({ action: 'pause' })
    const statusResult = await bridge.getStatus()

    expect(controlResult.ok).toBe(false)
    expect(controlResult.capability.enabled).toBe(false)
    expect(controlResult.error).toBeTruthy()
    expect(statusResult.ok).toBe(false)
    expect(statusResult.capability.enabled).toBe(false)
    expect(statusResult.error).toBeTruthy()
  })

  it('reports missing native addon candidates without throwing', () => {
    const result = loadEmbeddedMpvNativeAddon(['/missing/boxplayer-mpv-texture.node'])

    expect(result.addon).toBeUndefined()
    expect(result.error).toContain('未找到')
    expect(result.searchedPaths).toEqual(['/missing/boxplayer-mpv-texture.node'])
  })

  it('rejects native addon modules with an incomplete interface', () => {
    const dir = mkdtempSync(path.join(tmpdir(), 'boxplayer-mpv-addon-'))
    const addonPath = path.join(dir, 'incomplete.cjs')
    writeFileSync(addonPath, 'module.exports = { mpvTexture: {} }')

    const result = loadEmbeddedMpvNativeAddon([addonPath])

    expect(result.addon).toBeUndefined()
    expect(result.addonPath).toBe(addonPath)
    expect(result.error).toContain('接口不完整')
  })

  it('rejects native addon modules without control/status methods', () => {
    const dir = mkdtempSync(path.join(tmpdir(), 'boxplayer-mpv-addon-'))
    const addonPath = path.join(dir, 'missing-status.cjs')
    writeFileSync(addonPath, `
      module.exports = {
        mpvTexture: {
          create() {},
          load() {},
          play() {},
          pause() {},
          stop() {},
          seek() {},
          setVolume() {},
          setSpeed() {},
          setAudioTrack() {},
          setSubtitleTrack() {},
          addSubtitle() {},
          pollEvents() {},
          destroy() {},
          onFrame() {},
          onStatus() {},
          onError() {}
        }
      }
    `)

    const result = loadEmbeddedMpvNativeAddon([addonPath])

    expect(result.addon).toBeUndefined()
    expect(result.addonPath).toBe(addonPath)
    expect(result.error).toContain('接口不完整')
  })

  it('loads native addon modules with the expected mpvTexture shape', () => {
    const dir = mkdtempSync(path.join(tmpdir(), 'boxplayer-mpv-addon-'))
    mkdirSync(dir, { recursive: true })
    const addonPath = path.join(dir, 'complete.cjs')
    writeFileSync(addonPath, `
      module.exports = {
        mpvTexture: {
          create() {},
          load() {},
          play() {},
          pause() {},
          stop() {},
          seek() {},
          setVolume() {},
          setSpeed() {},
          setAudioTrack() {},
          setSubtitleTrack() {},
          addSubtitle() {},
          pollEvents() {},
          getStatus() {},
          getTrackStatus() {},
          destroy() {},
          onFrame() {},
          onStatus() {},
          onError() {},
          isInitialized() {}
        }
      }
    `)

    const result = loadEmbeddedMpvNativeAddon([addonPath])

    expect(result.addon).toBeTruthy()
    expect(result.addonPath).toBe(addonPath)
    expect(result.error).toBeUndefined()
  })
})
