import type { CreateMediaAcquisitionRunInput } from '@shared/types/mediaAcquisition'
import type { MediaLibraryFolder, MediaLibraryItem } from '../../types/media'
import { getMediaCoverage } from '../../utils/mediaCoverage'
import { getMediaAcquisitionCapability, normalizeMediaAcquisitionPlatform, normalizeMediaAcquisitionRootFolder, supportsMediaAcquisition } from './capabilities'

export interface HistoricalGapSettings {
  mediaAcquisitionPreferredQuality: 'auto' | '2160p' | '1080p' | '720p' | '480p'
  mediaAcquisitionFetchSubtitles: boolean
  mediaAcquisitionSubtitleLanguage: 'zh-CN' | 'zh-Hant' | 'en' | 'ja' | 'ko' | 'auto'
}

export interface HistoricalGapPlan extends CreateMediaAcquisitionRunInput {}

export function selectHistoricalScanBatch<T>(items: T[], cursor: number, limit = 200): { items: T[]; nextCursor: number } {
  if (!items.length || limit <= 0) return { items: [], nextCursor: 0 }
  const start = Math.max(0, cursor) % items.length
  const count = Math.min(limit, items.length)
  const batch = Array.from({ length: count }, (_, index) => items[(start + index) % items.length])
  return { items: batch, nextCursor: (start + count) % items.length }
}

export function buildHistoricalGapPlans(item: MediaLibraryItem, folder: MediaLibraryFolder, platform: string, settings: HistoricalGapSettings): HistoricalGapPlan[] {
  const capability = getMediaAcquisitionCapability(platform)
  const coverage = getMediaCoverage(item)
  if (!item.tmdbId || !folder.userId || !coverage || !supportsMediaAcquisition(capability)) return []
  return coverage.seasonGaps.map(gap => ({
    kind: 'missing',
    mediaLibraryItemId: item.id,
    tmdbId: item.tmdbId,
    mediaType: 'tv',
    title: item.name,
    year: item.year ? Number(item.year) : undefined,
    seasonNumber: gap.seasonNumber,
    missingEpisodes: gap.missingEpisodes,
    targetUserId: folder.userId!,
    targetDriveId: folder.driveId,
    targetPlatform: normalizeMediaAcquisitionPlatform(platform),
    targetParentFileId: normalizeMediaAcquisitionRootFolder(platform, folder.fileId),
    preferredQuality: settings.mediaAcquisitionPreferredQuality,
    fetchSubtitles: settings.mediaAcquisitionFetchSubtitles,
    preferredLanguage: settings.mediaAcquisitionFetchSubtitles ? settings.mediaAcquisitionSubtitleLanguage : undefined,
    trackingEnabled: true
  }))
}
