import type { MediaLibraryItem } from '../types/media'

/** Card progress uses 0–100; local playback records store a 0–1 fraction. */
export function mediaWatchProgressPercent(item: Pick<MediaLibraryItem, 'watchProgress' | 'lastPlayedPositionSeconds' | 'lastPlayedDurationSeconds'>): number {
  const position = item.lastPlayedPositionSeconds
  const duration = item.lastPlayedDurationSeconds
  if (Number.isFinite(position) && Number.isFinite(duration) && position! >= 0 && duration! > 0) {
    return Math.min(100, position! / duration! * 100)
  }
  const progress = Number(item.watchProgress)
  if (!Number.isFinite(progress) || progress <= 0) return 0
  return Math.min(100, progress <= 1 ? progress * 100 : progress)
}
