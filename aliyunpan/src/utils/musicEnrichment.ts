import useMusicLibraryStore from '../store/musiclibrary'
import { fetchMusicMetadata } from './musicMetadata'
import DebugLog from './debuglog'

const MAX_PARALLEL = 2
const BATCH_DELAY_MS = 800
const MAX_FAIL_PER_RUN = 12

let running = false
let stopRequested = false

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
  let enriched = 0
  let failures = 0
  try {
    const store = useMusicLibraryStore()
    // 选出无 cover_url 且未尝试过 enrich 或上次 enrich 距今 > 24h 的
    const now = Date.now()
    const candidates = store.tracks
      .filter((t) => !t.cover_url && (!t.enriched_at || now - t.enriched_at > 24 * 60 * 60 * 1000))
      .slice(0, maxItems)
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
              albumHint: t.album || ''
            })
            if (meta && (meta.cover || meta.album || meta.artist || meta.title)) {
              const patch: Record<string, unknown> = { enriched_at: Date.now() }
              if (meta.cover) patch.cover_url = meta.cover
              if (meta.album && !t.album) patch.album = meta.album
              if (meta.artist && !t.artist) patch.artist = meta.artist
              if (meta.title && !t.title) patch.title = meta.title
              await store.updateTrackEnrichment(t.id, patch)
              if (meta.cover) enriched += 1
            } else {
              // 标记尝试过，避免下次再选中
              await store.updateTrackEnrichment(t.id, { enriched_at: Date.now() })
              failures += 1
            }
          } catch (e) {
            failures += 1
            DebugLog.mSaveWarning('enrichMusicLibrary item failed: ' + (e as Error).message)
          }
          await new Promise((r) => setTimeout(r, BATCH_DELAY_MS))
        }
      })())
    }
    await Promise.all(workers)
  } finally {
    running = false
    stopRequested = false
  }
  return enriched
}
