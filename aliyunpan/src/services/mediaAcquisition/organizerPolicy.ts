import type { MediaAcquisitionTarget } from '@shared/types/mediaAcquisition'

const VIDEO_EXT = /\.(mp4|mkv|avi|mov|wmv|flv|webm|m4v|mpg|mpeg|3gp|rmvb|ts|m2ts|mts|vob)$/i
// Keep the same subtitle sidecar set as mediary-scout's movie finalizer.
const SUBTITLE_EXT = /\.(ass|ssa|srt|vtt|sub|idx|sup|smi)$/i
const NON_PRIMARY_VIDEO = /(?:\b(?:sample|trailer|teaser|featurette|behind[ ._-]?the[ ._-]?scenes|making[ ._-]?of|extras?|bonus|interview|commentary|preview|clip)\b|花絮|预告|样片|特典|制作特辑|采访|幕后)/i

export function isMediaAcquisitionPrimaryVideoName(name: string): boolean {
  return VIDEO_EXT.test(name) && !NON_PRIMARY_VIDEO.test(name)
}

export function isMediaAcquisitionSubtitleName(name: string): boolean {
  return SUBTITLE_EXT.test(name)
}

export function isMediaAcquisitionMatchingSidecar(subtitleName: string, videoNames: string[]): boolean {
  if (!isMediaAcquisitionSubtitleName(subtitleName)) return false
  const subtitleBase = subtitleName.replace(SUBTITLE_EXT, '').toLowerCase()
  return videoNames.some(videoName => {
    const videoBase = videoName.replace(VIDEO_EXT, '').toLowerCase()
    return videoBase.includes(subtitleBase) || subtitleBase.includes(videoBase)
  })
}

export function buildMediaAcquisitionTitleFolderName(target: Pick<MediaAcquisitionTarget, 'title' | 'year'>): string {
  const title = sanitizeFolderSegment(target.title || 'Unknown')
  return target.year ? `${title} (${target.year})` : title
}

export function buildMediaAcquisitionSeasonFolderName(seasonNumber?: number): string {
  const season = Number.isFinite(Number(seasonNumber)) && Number(seasonNumber) > 0 ? Number(seasonNumber) : 1
  return `Season ${season}`
}

export function buildMediaAcquisitionLeafPath(target: Pick<MediaAcquisitionTarget, 'title' | 'year' | 'mediaType' | 'seasonNumber'>): string[] {
  const titleFolder = buildMediaAcquisitionTitleFolderName(target)
  if (target.mediaType === 'movie') return [titleFolder]
  return [titleFolder, buildMediaAcquisitionSeasonFolderName(target.seasonNumber)]
}

function sanitizeFolderSegment(value: string): string {
  return value.replace(/[\\/:*?"<>|]/g, ' ').replace(/\s+/g, ' ').trim() || 'Unknown'
}
