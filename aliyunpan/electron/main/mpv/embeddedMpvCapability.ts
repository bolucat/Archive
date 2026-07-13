export type EmbeddedMpvStatus = 'available' | 'disabled' | 'unsupported-platform'

export interface EmbeddedMpvCapability {
  status: EmbeddedMpvStatus
  enabled: boolean
  platform: NodeJS.Platform | string
  electronVersion: string
  requirements: string[]
  reason?: string
}

export const MACOS_EMBEDDED_MPV_REQUIREMENTS = [
  'Electron sharedTexture API',
  'macOS libmpv native addon',
  'bundled libmpv.dylib and transitive dylibs'
]

export function getEmbeddedMpvCapability(options: {
  platform?: NodeJS.Platform | string
  electronVersion?: string
  featureFlag?: string
  nativeAddonAvailable?: boolean
  nativeResourcesComplete?: boolean
} = {}): EmbeddedMpvCapability {
  const platform = options.platform || process.platform
  const electronVersion = options.electronVersion || process.versions.electron || ''
  const nativeAddonAvailable = options.nativeAddonAvailable === true
  const nativeResourcesComplete = options.nativeResourcesComplete === true
  const base = {
    platform,
    electronVersion,
    requirements: MACOS_EMBEDDED_MPV_REQUIREMENTS
  }

  if (platform !== 'darwin') {
    return {
      ...base,
      status: 'unsupported-platform',
      enabled: false,
      reason: '内嵌 libmpv 仅支持 macOS；Windows/Linux 请使用自定义播放软件。'
    }
  }

  if (!nativeAddonAvailable) {
    return {
      ...base,
      status: 'disabled',
      enabled: false,
      reason: 'macOS libmpv 原生模块尚未接入或不可加载。'
    }
  }

  if (!nativeResourcesComplete) {
    return {
      ...base,
      status: 'disabled',
      enabled: false,
      reason: 'macOS libmpv 资源包不完整，尚不能启用内嵌 MPV。'
    }
  }

  return {
    ...base,
    status: 'available',
    enabled: true
  }
}
