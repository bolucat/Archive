import Config from '../../config'
import useSettingStore from '../../setting/settingstore'
import { useMediaLibraryStore } from '../../store/medialibrary'
import type { MediaLibraryItem } from '../../types/media'
import UserDAL from '../../user/userdal'
import { buildExpectedSeasons } from '../../utils/mediaCoverage'
import { createMediaAcquisitionRun } from './client'
import { buildHistoricalGapPlans, selectHistoricalScanBatch } from './historicalPolicy'
import { isPro } from '../../utils/usageLimit'

const HISTORICAL_SCAN_INTERVAL_MS = 6 * 60 * 60 * 1000
let lastScanAt = 0
let scanning = false
let scanCursor = 0

export async function runHistoricalMediaLibraryGapScan(options: { force?: boolean } = {}): Promise<number> {
  const settings = useSettingStore()
  if (!isPro() || !settings.mediaAcquisitionAutoScanHistorical || scanning) return 0
  const now = Date.now()
  if (!options.force && now - lastScanAt < HISTORICAL_SCAN_INTERVAL_MS) return 0
  scanning = true
  lastScanAt = now
  let created = 0
  try {
    const store = useMediaLibraryStore()
    const folders = new Map(store.folders.map(folder => [folder.id, folder]))
    const candidates = store.mediaItems.filter(item => item.type === 'tv' && item.tmdbId)
    const batch = selectHistoricalScanBatch(candidates, scanCursor)
    const scanItems = options.force ? candidates : batch.items
    if (!options.force) scanCursor = batch.nextCursor
    for (const original of scanItems) {
      const item = await refreshHistoricalMetadata(original)
      if (item !== original) store.addMediaItem(item)
      const firstFile = item.driveFiles?.[0]
      const folder = (item.folderId ? folders.get(item.folderId) : undefined)
        || store.folders.find(candidate => candidate.userId === firstFile?.userId && candidate.driveId === firstFile?.driveId)
      if (!folder?.userId) continue
      const platform = UserDAL.GetUserToken(folder.userId)?.tokenfrom || folder.driveServerId
      for (const plan of buildHistoricalGapPlans(item, folder, platform, settings)) {
        try {
          await createMediaAcquisitionRun(plan)
          created += 1
        } catch (error: any) {
          if (!/已有|不能重复获取/.test(String(error?.message || ''))) console.warn('[media-acquisition] historical gap task skipped', item.name, error?.message || error)
        }
      }
      await new Promise(resolve => window.setTimeout(resolve, 0))
    }
  } finally {
    scanning = false
  }
  return created
}

async function refreshHistoricalMetadata(item: MediaLibraryItem): Promise<MediaLibraryItem> {
  try {
    const base = Config.BOXPLAYER_API_URL.replace(/\/+$/, '')
    const response = await fetch(`${base}/api/tmdb/proxy/tv/${item.tmdbId}?language=zh-CN`)
    if (!response.ok) return item
    const payload = await response.json()
    const tv = payload?.data || payload
    const expectedSeasons = buildExpectedSeasons({ tv })
    return expectedSeasons.length ? { ...item, expectedSeasons } : item
  } catch {
    return item
  }
}
