export type RenderQualityMode = 'auto' | 'keep' | 'release'

export function getRenderPixelRatio(mode: RenderQualityMode, hidden = document.hidden) {
  const dpr = window.devicePixelRatio || 1
  if (mode === 'release' || hidden) return Math.min(dpr, 1)
  if (mode === 'keep') return Math.min(dpr, 2)
  return Math.min(dpr, 1.5)
}

export function getParticleDensityScale(mode: RenderQualityMode, hidden = document.hidden) {
  if (mode === 'release' || hidden) return 1.45
  if (mode === 'keep') return 0.88
  return 1
}
