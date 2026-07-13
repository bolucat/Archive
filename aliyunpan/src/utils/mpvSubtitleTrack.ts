export interface MpvSubtitleTrack {
  id: number
  type: string
  external?: boolean
}

export function getAutoSubtitleTrackId(tracks: MpvSubtitleTrack[], subtitleTrackId: number, hasExternalSubtitle: boolean): number | undefined {
  if (hasExternalSubtitle || subtitleTrackId >= 0) return undefined
  return tracks.find((track) => track.type === 'sub' && !track.external)?.id
}
