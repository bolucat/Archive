import useMusicLibraryStore from '../store/musiclibrary'
import { fetchMusicMetadata } from './musicMetadata'
import DebugLog from './debuglog'

const MAX_PARALLEL = 2
const BATCH_DELAY_MS = 800
const MAX_FAIL_PER_RUN = 12

let running = false
let stopRequested = false
const retryAfterByTrackId = new Map<string, number>()

export function isMusicEnrichmentRunning(): boolean {
  return running
}

export function stopMusicEnrichment(): void {
  stopRequested = true
}

/**
 * 懒加载补全 IMusicTrack 的 cover_url / album / artist / title。
 * 跑在后台，按文件名去 LRCLIB / iTunes 查询，每次最多取 N 首没有 cover_url 的曲目。
 * 同一进程内幂等：已 running 时直接返回。
 */
export async function enrichMusicLibrary(maxItems: number = 60): Promise<number> {
  if (running) return 0
  running = true
  stopRequested = false
  let attempted = 0
  let failures = 0
  try {
    const store = useMusicLibraryStore()
    const now = Date.now()
    for (const [id, retryAt] of retryAfterByTrackId) {
      if (retryAt <= now) retryAfterByTrackId.delete(id)
    }
    const candidates = await store.getEnrichmentCandidates(maxItems, now, new Set(retryAfterByTrackId.keys()))
    if (!candidates.length) return 0

    const queue = [...candidates]
    const workers: Promise<void>[] = []
    for (let i = 0; i < Math.min(MAX_PARALLEL, queue.length); i++) {
      workers.push((async () => {
        while (queue.length && !stopRequested && failures < MAX_FAIL_PER_RUN) {
          const t = queue.shift()
          if (!t) break
          try {
            const meta = await fetchMusicMetadata({
              filename: t.file_name,
              artistHint: t.artist || '',
              titleHint: t.title || '',
              albumHint: t.album || '',
              includeLyrics: false
            })
            const hasExternalMetadata = !!meta?.metadataSources?.some((source) => source !== 'filename')
            if (meta && hasExternalMetadata) {
              const patch: Record<string, unknown> = { enriched_at: Date.now() }
              if (meta.cover) patch.cover_url = meta.cover
              const hasITunesMetadata = meta.metadataSources?.includes('itunes:metadata')
              if (hasITunesMetadata && t.metadata_source !== 'manual') {
                if (meta.album) patch.album = meta.album
                if (meta.artist) patch.artist = meta.artist
                if (meta.title) patch.title = meta.title
                patch.metadata_source = 'itunes'
              }
              await store.updateTrackEnrichment(t.id, patch)
              retryAfterByTrackId.delete(t.id)
            } else {
              // 标记尝试过，避免下次再选中
              await store.updateTrackEnrichment(t.id, { enriched_at: Date.now() })
              failures += 1
            }
          } catch (e) {
            failures += 1
            retryAfterByTrackId.set(t.id, Date.now() + 60_000)
            DebugLog.mSaveWarning('enrichMusicLibrary item failed: ' + (e as Error).message)
          }
          attempted += 1
          await new Promise((r) => setTimeout(r, BATCH_DELAY_MS))
        }
      })())
    }
    await Promise.all(workers)
  } finally {
    running = false
    stopRequested = false
  }
  return attempted
}
