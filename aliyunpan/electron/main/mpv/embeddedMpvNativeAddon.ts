import { existsSync, readFileSync } from 'fs'
import path from 'path'
import { createRequire } from 'module'
import { getResourcesPath, getStaticPath } from '../utils/mainfile'

export interface EmbeddedMpvTextureInfo {
  handle: bigint
  width: number
  height: number
  format: 'rgba' | 'bgra' | 'nv12' | string
}

export interface EmbeddedMpvStatus {
  playing?: boolean
  paused?: boolean
  position?: number
  duration?: number
  volume?: number
  speed?: number
  width?: number
  height?: number
}

export interface EmbeddedMpvTrack {
  id: number
  type: string
  title?: string
  language?: string
  codec?: string
  selected?: boolean
  external?: boolean
}

export interface EmbeddedMpvTrackStatus {
  audioId?: number
  subtitleId?: number
  tracks?: EmbeddedMpvTrack[]
}

export interface EmbeddedMpvSubtitleStyle {
  fontSize?: number
  color?: string
  position?: number
  bold?: boolean
  italic?: boolean
}

export interface EmbeddedMpvNativeInstance {
  create(config?: Record<string, unknown>): void
  load(url: string, options?: string): Promise<void> | void
  play(): void
  pause(): void
  stop(): void
  seek(position: number): void
  setVolume(volume: number): void
  setSpeed?: (speed: number) => void
  setAudioTrack?: (id: number) => void
  setSubtitleTrack?: (id: number) => void
  setSubtitleStyle?: (style: EmbeddedMpvSubtitleStyle) => void
  setVideoProperty?: (name: string, value: string) => void
  addAudio?: (url: string, title?: string) => void
  addSubtitle?: (url: string, title?: string) => void
  pollEvents?: () => void
  getStatus(): EmbeddedMpvStatus
  getTrackStatus?: () => EmbeddedMpvTrackStatus
  destroy(): void
  onFrame(callback: (textureInfo: EmbeddedMpvTextureInfo) => void): void
  onStatus(callback: (status: EmbeddedMpvStatus) => void): void
  onError(callback: (error: string) => void): void
  releaseFrame?: () => void
  isInitialized?: () => boolean
}

export interface EmbeddedMpvNativeAddon {
  mpvTexture: EmbeddedMpvNativeInstance
}

export interface EmbeddedMpvNativeAddonLoadResult {
  addon?: EmbeddedMpvNativeAddon
  addonPath?: string
  error?: string
  searchedPaths: string[]
}

export interface EmbeddedMpvNativeResourceStatus {
  complete: boolean
  directory?: string
  missing: string[]
  error?: string
}

const requireNative = createRequire(import.meta.url)

export function getEmbeddedMpvNativeAddonCandidates(platform = process.platform, arch = process.arch): string[] {
  if (platform !== 'darwin') return []
  const sbtlRelativePath = path.join('engine', 'darwin', arch, 'mpv-texture', 'mpv_texture.node')
  const legacyRelativePath = path.join('engine', 'darwin', arch, 'mpv-texture', 'boxplayer-mpv-texture.node')
  return [
    getResourcesPath(sbtlRelativePath),
    getStaticPath(sbtlRelativePath),
    getResourcesPath(legacyRelativePath),
    getStaticPath(legacyRelativePath)
  ]
}

export function getEmbeddedMpvNativeResourceStatus(candidates = getEmbeddedMpvNativeAddonCandidates()): EmbeddedMpvNativeResourceStatus {
  const missing: string[] = []

  for (const candidate of candidates) {
    if (!existsSync(candidate)) {
      missing.push(candidate)
      continue
    }

    const directory = path.dirname(candidate)
    const libmpvPath = path.join(directory, 'libmpv.dylib')
    const manifestPath = path.join(directory, 'mpv-bundle-manifest.json')
    const directoryMissing = [libmpvPath, manifestPath].filter((filePath) => !existsSync(filePath))
    if (directoryMissing.length > 0) {
      return {
        complete: false,
        directory,
        missing: directoryMissing
      }
    }

    try {
      const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
      const files = Array.isArray(manifest?.files) ? manifest.files : []
      const hasNode = files.some((file: any) => file?.name === 'mpv_texture.node' || file?.name === 'boxplayer-mpv-texture.node')
      const hasLibmpv = files.some((file: any) => file?.name === 'libmpv.dylib')
      if (!hasNode || !hasLibmpv) {
        return {
          complete: false,
          directory,
          missing: [
            ...(hasNode ? [] : ['mpv-bundle-manifest.json:mpv_texture.node']),
            ...(hasLibmpv ? [] : ['mpv-bundle-manifest.json:libmpv.dylib'])
          ]
        }
      }
    } catch (error: any) {
      return {
        complete: false,
        directory,
        missing: [],
        error: error?.message || 'macOS MPV resource manifest 读取失败。'
      }
    }

    return {
      complete: true,
      directory,
      missing: []
    }
  }

  return {
    complete: false,
    missing
  }
}

function hasSbtlMpvTextureShape(mpvTexture: any): mpvTexture is EmbeddedMpvNativeInstance {
  return Boolean(
    mpvTexture &&
      typeof mpvTexture.create === 'function' &&
      typeof mpvTexture.load === 'function' &&
      typeof mpvTexture.play === 'function' &&
      typeof mpvTexture.pause === 'function' &&
      typeof mpvTexture.stop === 'function' &&
      typeof mpvTexture.seek === 'function' &&
      typeof mpvTexture.setVolume === 'function' &&
      typeof mpvTexture.getStatus === 'function' &&
      typeof mpvTexture.destroy === 'function' &&
      typeof mpvTexture.onFrame === 'function' &&
      typeof mpvTexture.onStatus === 'function' &&
      typeof mpvTexture.onError === 'function'
  )
}

function normalizeMpvStatus(status: EmbeddedMpvStatus | undefined): EmbeddedMpvStatus {
  const playing = Boolean(status?.playing)
  return {
    ...(status || {}),
    playing,
    paused: typeof status?.paused === 'boolean' ? status.paused : !playing,
    speed: typeof status?.speed === 'number' ? status.speed : 1
  }
}

function toEmbeddedMpvNativeAddon(value: any): EmbeddedMpvNativeAddon | null {
  const mpvTexture = hasSbtlMpvTextureShape(value?.mpvTexture) ? value.mpvTexture : hasSbtlMpvTextureShape(value) ? value : null
  if (!mpvTexture) return null
  return {
    mpvTexture: {
      ...mpvTexture,
      load(url: string, options?: string) {
        return mpvTexture.load(url, options)
      },
      getStatus() {
        return normalizeMpvStatus(mpvTexture.getStatus?.())
      },
      getTrackStatus() {
        return mpvTexture.getTrackStatus?.() || { audioId: -1, subtitleId: -1, tracks: [] }
      },
      onStatus(callback: (status: EmbeddedMpvStatus) => void) {
        mpvTexture.onStatus((status) => callback(normalizeMpvStatus(status)))
      },
      setSpeed: mpvTexture.setSpeed,
      setAudioTrack: mpvTexture.setAudioTrack,
      setSubtitleTrack: mpvTexture.setSubtitleTrack,
      setSubtitleStyle: mpvTexture.setSubtitleStyle,
      setVideoProperty: mpvTexture.setVideoProperty,
      addAudio: mpvTexture.addAudio,
      addSubtitle: mpvTexture.addSubtitle,
      pollEvents: mpvTexture.pollEvents,
      releaseFrame: mpvTexture.releaseFrame
    }
  }
}

export function loadEmbeddedMpvNativeAddon(candidates = getEmbeddedMpvNativeAddonCandidates()): EmbeddedMpvNativeAddonLoadResult {
  for (const candidate of candidates) {
    if (!existsSync(candidate)) continue
    try {
      const addon = toEmbeddedMpvNativeAddon(requireNative(candidate))
      if (!addon) {
        return {
          addonPath: candidate,
          error: 'macOS MPV native addon 接口不完整。',
          searchedPaths: candidates
        }
      }
      return {
        addon,
        addonPath: candidate,
        searchedPaths: candidates
      }
    } catch (error: any) {
      return {
        addonPath: candidate,
        error: error?.message || 'macOS MPV native addon 加载失败。',
        searchedPaths: candidates
      }
    }
  }

  return {
    error: '未找到 macOS MPV native addon。',
    searchedPaths: candidates
  }
}
