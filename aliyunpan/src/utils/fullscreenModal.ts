/** A native fullscreen page only renders descendants of its fullscreen root. */
export const resolveFullscreenModalContainer = <T>(isFullscreen: boolean, fullscreenElement: T | null | undefined): T | undefined => {
  return isFullscreen ? fullscreenElement || undefined : undefined
}
