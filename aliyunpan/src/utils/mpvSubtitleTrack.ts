export interface MpvSubtitleTrack {
  id: number
  type: string
  external?: boolean
}

export function getAutoSubtitleTrackId(tracks: MpvSubtitleTrack[], subtitleTrackId: number, hasExternalSubtitle: boolean): number | undefined {
  if (hasExternalSubtitle || subtitleTrackId >= 0) return undefined
  return tracks.find((track) => track.type === 'sub' && !track.external)?.id
}

export function subtitleSelectionCommands(secondary: boolean, nextId: number, primaryId: number, secondaryId: number): Array<{ secondary: boolean; id: number }> {
  // libmpv cannot use a single track in both subtitle slots. Move it between slots.
  if (secondary) return [{ secondary: false, id: primaryId === nextId && nextId >= 0 ? -1 : primaryId }, { secondary: true, id: nextId }]
  return [...(nextId >= 0 && nextId === secondaryId ? [{ secondary: true, id: -1 }] : []), { secondary: false, id: nextId }]
}

// MPV scales text against a 720px reference height. Keep the secondary block
// below the primary block; the position slider moves the entire pair.
export function bilingualSubtitlePositions(position: number, fontSize: number, scale: number, dual: boolean, secondaryLines = 1) {
  const secondary = Math.max(0, Math.min(100, position))
  const lineHeight = Math.max(1, fontSize) * Math.max(0.25, scale) * 1.25
  const gap = (lineHeight * Math.max(1, secondaryLines) + 8) / 720 * 100
  return { primary: dual ? Math.max(0, secondary - gap) : secondary, secondary }
}
