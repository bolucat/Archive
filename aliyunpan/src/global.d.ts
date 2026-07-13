export {}

declare global {
  // eslint-disable-next-line no-unused-vars
  interface Window {
    Go: any
    require: any
    Electron: any
    openDatabase: any
    WebRelaunchAria: () => Promise<number>
    platform: string
    WinMsg: any
    postdataFunc: any
    Prism: any
    WebUserToken: any
    WebToElectron: any
    WebToWindow: any
    WebClearCache: any
    WebRelaunch: any
    WebGetCookies: any
    WebQuarkAccountInfo: any
    WebSetCookies: any
    WebClearCookies: any
    WebShutDown: any
    WebOpenWindow: any
    WebOpenLyric: () => void
    WebSendLyric: (data: any) => void
    WebCloseLyric: () => void
    WebConfigureGlobalHotkeys: (data: any) => Promise<any>
    WebOnGlobalHotkey: (callback: (data: any) => void) => (() => void)
    WebOpenUrl: any
    WebShowOpenDialogSync: any
    WebShowItemInFolder: any
    WebExecSync: any
    WebSpawnSync: any
    WebPlatformSync: any
    UploadPort: any
    DownloadPort: any
    MainPort: any
    MainProxyServer: any
    MainProxyHost: any
    MainProxyPort: any
    WinMsgToUpload: any
    WinMsgToDownload: any
    WinMsgToMain: any
    IsMainPage: boolean
    WebSetProxy: any
    WebMpvEmbeddedCapability: () => Promise<any>
    WebMpvEmbeddedLoad: (data: any) => Promise<any>
    WebMpvEmbeddedControl: (data: any) => Promise<any>
    WebMpvEmbeddedStatus: () => Promise<any>
    WebMpvSharedTextureCapability: () => { available: boolean; platform: string; reason?: string }
    WebMpvSharedTexture: {
      isAvailable: () => boolean
      onFrame: (callback: (videoFrame: VideoFrame, index: number) => void) => void
      removeFrameListener: () => void
      onClear: (callback: () => void) => void
      removeClearListener: () => void
    }
    speedLimte: number
    WebSetProgressBar: any
    TvBoxInvoke: (channel: string, data: unknown) => Promise<unknown>
    ReedyInvoke: (channel: string, ...args: any[]) => Promise<any>
    onExternalDownloadOpen: (callback: (payload: string) => void) => void
    MsImageCacheSyncConfig: any
    MsImageCacheStats: () => Promise<{ totalBytes: number; servers: unknown[] }>
    MsImageCacheClear: (serverId?: string) => Promise<{ cleared: number }>
  }
}
