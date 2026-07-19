import type { MediaAcquisitionFileSnapshot, MediaAcquisitionTarget } from '@shared/types/mediaAcquisition'

const VIDEO_EXT = /\.(mp4|mkv|avi|mov|wmv|flv|webm|m4v|mpg|mpeg|3gp|rmvb|ts|m2ts|mts|vob)$/i
const SUBTITLE_EXT = /\.(ass|ssa|srt|vtt)$/i

export function pickSubtitleReferenceVideo(files: MediaAcquisitionFileSnapshot[]): MediaAcquisitionFileSnapshot | undefined {
  return files
    .filter(file => VIDEO_EXT.test(file.name) && (file.size === undefined || file.size > 0))
    .sort((left, right) => (right.size || 0) - (left.size || 0))[0]
}

export function buildSubtitleSearchQuery(target: MediaAcquisitionTarget, files: MediaAcquisitionFileSnapshot[] = []): string {
  const reference = pickSubtitleReferenceVideo(files)
  if (reference?.name) return stripExtension(reference.name).replace(/[._]+/g, ' ').replace(/\s+/g, ' ').trim()
  return [target.title, target.year].filter(Boolean).join(' ')
}

export function buildSidecarSubtitleName(videoName: string | undefined, subtitleName: string): string {
  const subtitleExt = subtitleName.match(SUBTITLE_EXT)?.[0] || '.srt'
  const base = videoName ? stripExtension(videoName) : stripExtension(subtitleName)
  return `${base}${subtitleExt.toLowerCase()}`
}

/** Unknown origin is intentionally not treated as foreign: subtitle fetching is opt-in by evidence. */
export function isConfirmedNonDomesticMediaOrigin(countries: string[]): boolean {
  return countries.length > 0 && countries.every(country => country.toUpperCase() !== 'CN')
}

function stripExtension(name: string): string {
  return name.replace(/\.[^.\\/]+$/, '')
}
