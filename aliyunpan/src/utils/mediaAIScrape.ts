import DebugLog from './debuglog'
import message from './message'
import { resolveAIProviderConfig } from './bookAI'
import { isBoxPlayerCloudProvider, scrapeMediaWithBoxPlayerCloud, type BoxPlayerCloudMediaScrapeResult } from './boxplayerCloudAI'
import { checkAndIncrement, isPro } from './usageLimit'
import { TmdbService, tmdbImageUrl } from './tmdb'
import type { DriveFileItem, MediaEpisode, MediaLibraryItem, MediaLibraryTvSeriesItem, MovieItem } from '../types/media'

export interface MediaAIScrapeInput {
  file?: DriveFileItem
  item?: MediaLibraryItem
  folderName?: string
  folderId?: string
  folderContext?: string[]
  currentMatch?: {
    type?: MediaLibraryItem['type']
    name?: string
    tmdbId?: number
    year?: string
  }
}

export interface MediaAIScrapeDecision {
  type: 'movie' | 'tv' | 'unknown'
  title: string
  year?: number
  season?: number
  episode?: number
  tmdbId?: number
  confidence: number
  allowOverwrite: boolean
  reason: string
}

export interface MediaAIScrapeResult {
  ok: boolean
  decision?: MediaAIScrapeDecision
  provider?: string
  error?: string
}

export interface AIScrapeApplyOptions {
  tmdb?: Pick<TmdbService, 'searchMovie' | 'searchTV'>
  now?: () => Date
}

const MIN_CONFIDENCE = 0.65

export function canUseMediaAIScrape(options: { manual: boolean }): { allowed: boolean, message?: string } {
  if (options.manual) return checkAndIncrement('mediaAIScrape', 1, { metered: false })
  if (!isPro()) return { allowed: false }
  return { allowed: true }
}

export function parseMediaAIScrapeDecision(text: string): MediaAIScrapeDecision {
  const raw = extractJson(text)
  const parsed = JSON.parse(raw)
  const normalized = normalizeMediaAIScrapeDecision(parsed)
  if (!normalized.decision) throw new Error(normalized.reason || 'AI 未返回有效刮削决策')
  return normalized.decision
}

export function normalizeMediaAIScrapeDecision(value: unknown): { usable: boolean, decision?: MediaAIScrapeDecision, reason?: string } {
  const raw = value as Record<string, unknown>
  const type = raw?.type
  if (type !== 'movie' && type !== 'tv' && type !== 'unknown') return { usable: false, reason: 'AI 返回了无效媒体类型' }
  const confidence = clampNumber(raw.confidence, 0, 1)
  const title = String(raw.title || '').trim()
  const decision: MediaAIScrapeDecision = {
    type,
    title,
    year: optionalInt(raw.year),
    season: optionalInt(raw.season),
    episode: optionalInt(raw.episode),
    tmdbId: optionalInt(raw.tmdbId),
    confidence,
    allowOverwrite: raw.allowOverwrite === true,
    reason: String(raw.reason || '').slice(0, 160)
  }
  if (decision.type === 'unknown') return { usable: false, decision, reason: 'AI 无法识别该文件' }
  if (!decision.title) return { usable: false, decision, reason: 'AI 未返回片名' }
  if (decision.confidence < MIN_CONFIDENCE) return { usable: false, decision, reason: `AI 置信度过低：${decision.confidence}` }
  if (decision.type === 'tv' && (!decision.season || !decision.episode)) return { usable: false, decision, reason: 'AI 未返回剧集季集信息' }
  return { usable: true, decision }
}

export async function scrapeMediaWithAI(input: MediaAIScrapeInput, options: { manual?: boolean, timeoutMs?: number } = {}): Promise<MediaAIScrapeResult> {
  const cfg = resolveAIProviderConfig()
  const gate = canUseMediaAIScrape({ manual: options.manual === true })
  if (!gate.allowed) return { ok: false, error: gate.message || 'AI 影视刮削不可用' }
  if (!cfg || !isBoxPlayerCloudProvider(cfg.providerName)) return { ok: false }

  try {
    DebugLog.mSaveLog('info', '[MediaAIScrape] input ' + JSON.stringify(buildInputLog(input)), undefined)
    const decision = await scrapeSingleMediaWithBoxPlayerCloud(input, options.timeoutMs || 20000)
    const normalized = normalizeMediaAIScrapeDecision(decision)
    if (!normalized.usable || !normalized.decision) return { ok: false, decision, provider: cfg.providerName, error: normalized.reason || 'AI 决策不可用' }
    return { ok: true, decision: normalized.decision, provider: cfg.providerName }
  } catch (error: any) {
    const msg = error?.message || String(error)
    DebugLog.mSaveWarning('[MediaAIScrape] failed ' + msg)
    return { ok: false, provider: cfg.providerName, error: msg }
  }
}

export async function applyAIScrapeResult(input: MediaAIScrapeInput, result: MediaAIScrapeResult, options: AIScrapeApplyOptions = {}): Promise<MediaLibraryItem | null> {
  const decision = result.decision
  if (!result.ok || !decision) return null
  const files = resolveDriveFiles(input)
  if (!files.length) return null
  const file = files[0]
  const folderName = input.folderName || input.item?.parentId || file.path.split('/').slice(-2, -1)[0] || ''
  const folderId = input.folderId || input.item?.folderId
  const folderPath = input.item?.folderPath || file.path.substring(0, file.path.lastIndexOf('/')) || ''
  const tmdb = options.tmdb || TmdbService.getInstance()
  const now = options.now?.() || new Date()

  if (decision.type === 'movie') {
    const movie = await tmdb.searchMovie(decision.title, decision.year ? String(decision.year) : undefined, undefined, file.fileHash || file.contentHash, file.name)
    if (!movie) return null
    return decorateAIScrapeItem(movieToMediaItem(movie, files, folderName, folderId, folderPath, now), result, input.item)
  }

  if (decision.type === 'tv') {
    const tv = await tmdb.searchTV(decision.title, decision.season || 1, decision.year ? String(decision.year) : undefined, undefined, file.fileHash || file.contentHash, file.name)
    if (!tv) return null
    const item = tvToMediaItem(tv, decision, files, folderName, folderId, folderPath, now)
    return item ? decorateAIScrapeItem(item, result, input.item) : null
  }

  return null
}

export async function manualAIScrapeItem(item: MediaLibraryItem): Promise<MediaLibraryItem | null> {
  const result = await scrapeMediaWithAI({ item, currentMatch: mediaItemToCurrentMatch(item) }, { manual: true })
  if (!result.ok) {
    if (result.error) message.error(result.error)
    return null
  }
  const applied = await applyAIScrapeResult({ item, currentMatch: mediaItemToCurrentMatch(item) }, result)
  if (!applied) {
    message.error('AI 重刮削未匹配到 TMDB 结果')
    return null
  }
  return applied
}

export async function manualAIScrapeItems(item: MediaLibraryItem): Promise<MediaLibraryItem[]> {
  const files = resolveDriveFiles({ item })
  if (item.type !== 'tv' && files.length <= 1) {
    const single = await manualAIScrapeItem(item)
    return single ? [single] : []
  }

  const results = await batchScrapeMediaWithAI(
    files.map(file => ({
      name: file.name,
      path: file.path,
      fileSize: file.fileSize,
      driveFile: file
    })),
    item.name
  )
  const scraped = results
    .map(result => result.mediaItem)
    .filter((mediaItem): mediaItem is MediaLibraryItem => !!mediaItem)

  if (!scraped.length) {
    const firstError = results.find(result => result.error)?.error
    if (firstError) message.error(firstError)
  }
  return scraped
}

function movieToMediaItem(movie: MovieItem, files: DriveFileItem[], folderName: string, folderId: string | undefined, folderPath: string, addedAt: Date): MediaLibraryItem {
  return {
    id: `${files[0].id}`,
    parentId: folderName,
    folderId,
    folderPath,
    type: 'movie',
    name: movie.title || movie.original_title,
    overview: movie.overview,
    posterUrl: tmdbImageUrl(movie.poster_path) || undefined,
    backdropUrl: tmdbImageUrl(movie.backdrop_path) || undefined,
    year: movie.release_date?.substring(0, 4),
    rating: movie.vote_average,
    genres: movie.genres?.map(g => g.name) || [],
    credits: movie.credits,
    productionCountries: movie.production_countries?.map(c => c.name) || [],
    tmdbId: movie.id,
    imdbId: movie.imdb_id,
    driveFiles: files,
    addedAt
  }
}

function tvToMediaItem(tvResult: MediaLibraryTvSeriesItem, decision: MediaAIScrapeDecision, files: DriveFileItem[], folderName: string, folderId: string | undefined, folderPath: string, addedAt: Date): MediaLibraryItem | null {
  const season = tvResult.current_season
  const matchedEpisode = season?.episodes?.find(ep => ep.episode_number === decision.episode)
  if (!season || !matchedEpisode) return null
  const episode: MediaEpisode = {
    id: matchedEpisode.id,
    episodeNumber: matchedEpisode.episode_number,
    seasonNumber: matchedEpisode.season_number,
    name: matchedEpisode.name,
    overview: matchedEpisode.overview,
    stillPath: tmdbImageUrl(matchedEpisode.still_path) || undefined,
    airDate: matchedEpisode.air_date,
    runtime: matchedEpisode.runtime,
    crew: matchedEpisode.crew,
    driveFiles: files
  }
  return {
    id: `${tvResult.tv.id}`,
    parentId: folderName,
    folderId,
    folderPath,
    type: 'tv',
    name: tvResult.tv.name || tvResult.tv.original_name || decision.title,
    overview: tvResult.tv.overview,
    posterUrl: tmdbImageUrl(tvResult.tv.poster_path) || undefined,
    backdropUrl: tmdbImageUrl(tvResult.tv.backdrop_path) || undefined,
    year: tvResult.tv.first_air_date?.substring(0, 4),
    rating: tvResult.tv.vote_average,
    genres: tvResult.tv.genres?.map(g => g.name) || [],
    credits: season.credits || tvResult.tv.credits,
    productionCountries: tvResult.tv.production_countries?.map(c => c.name) || [],
    tmdbId: tvResult.tv.id,
    imdbId: tvResult.tv.imdbId,
    tvdbId: tvResult.tv.tvdbId,
    seasons: [{
      id: season.id,
      seasonNumber: season.season_number,
      name: season.name,
      overview: season.overview,
      posterPath: season.poster_path,
      episodeCount: season.episode_count || season.episodes?.length || 0,
      airDate: season.air_date,
      credits: season.credits,
      episodes: [episode]
    }],
    driveFiles: [],
    addedAt
  }
}

function decorateAIScrapeItem(item: MediaLibraryItem, result: MediaAIScrapeResult, existing?: MediaLibraryItem): MediaLibraryItem {
  const preserved = existing ? {
    lastWatched: existing.lastWatched,
    watchProgress: existing.watchProgress,
    lastPlayedFileId: existing.lastPlayedFileId,
    addedAt: existing.addedAt || item.addedAt
  } : {}
  return {
    ...item,
    ...preserved,
    aiScrape: {
      provider: result.provider || 'unknown',
      confidence: result.decision?.confidence || 0,
      reason: result.decision?.reason || '',
      updatedAt: Date.now()
    },
    metadataSource: 'ai-tmdb'
  } as MediaLibraryItem
}

function resolveDriveFiles(input: MediaAIScrapeInput): DriveFileItem[] {
  if (input.file) return [input.file]
  if (input.item?.driveFiles?.length) return input.item.driveFiles
  return input.item?.seasons
    ?.flatMap(season => season.episodes || [])
    .flatMap(episode => episode.driveFiles || []) || []
}

function mediaItemToCurrentMatch(item: MediaLibraryItem): MediaAIScrapeInput['currentMatch'] {
  return {
    type: item.type,
    name: item.name,
    tmdbId: item.tmdbId,
    year: item.year
  }
}

function extractJson(text: string): string {
  const trimmed = String(text || '').trim()
  const fenced = trimmed.match(/```(?:json)?\s*([\s\S]*?)```/i)
  if (fenced) return fenced[1].trim()
  const start = trimmed.indexOf('{')
  const end = trimmed.lastIndexOf('}')
  if (start >= 0 && end > start) return trimmed.slice(start, end + 1)
  return trimmed
}

function optionalInt(value: unknown): number | undefined {
  if (value === undefined || value === null || value === '') return undefined
  const n = typeof value === 'number' ? value : parseInt(String(value), 10)
  return Number.isFinite(n) ? n : undefined
}

function clampNumber(value: unknown, min: number, max: number): number {
  const n = typeof value === 'number' ? value : parseFloat(String(value || '0'))
  if (!Number.isFinite(n)) return min
  return Math.max(min, Math.min(max, n))
}

function buildInputLog(input: MediaAIScrapeInput): Record<string, unknown> {
  const file = input.file || input.item?.driveFiles?.[0]
  return {
    fileName: file?.name || input.item?.name,
    path: file?.path,
    folderName: input.folderName || input.item?.parentId,
    currentMatch: input.currentMatch
  }
}

function withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('AI 刮削超时')), timeoutMs)
    promise.then(
      (value) => {
        clearTimeout(timer)
        resolve(value)
      },
      (error) => {
        clearTimeout(timer)
        reject(error)
      }
    )
  })
}

export interface BatchScrapeFileInfo {
  name: string
  path: string
  fileSize?: number
  driveFile?: DriveFileItem
}

export interface BatchScrapeResult {
  file: BatchScrapeFileInfo
  decision?: MediaAIScrapeDecision | null
  mediaItem?: MediaLibraryItem | null
  error?: string
}

export async function batchScrapeMediaWithAI(files: BatchScrapeFileInfo[], folderName: string): Promise<BatchScrapeResult[]> {
  if (!files.length) return []

  const cfg = resolveAIProviderConfig()
  const gate = canUseMediaAIScrape({ manual: true })
  if (!gate.allowed) {
    return files.map(f => ({ file: f, error: gate.message || 'AI 影视刮削不可用' }))
  }

  if (!cfg || !isBoxPlayerCloudProvider(cfg.providerName)) {
    return files.map(file => ({ file }))
  }

  try {
    DebugLog.mSaveLog('info', '[BatchAIScrape] input ' + files.length + ' files, folder: ' + folderName, undefined)
    const decisions = await scrapeBatchMediaWithBoxPlayerCloud(files, folderName)
    const tmdb = TmdbService.getInstance()
    const now = new Date()

    const results: BatchScrapeResult[] = []
    for (let i = 0; i < files.length; i++) {
      const file = files[i]
      const decision = decisions[i]
      if (!decision || decision.type === 'unknown') {
        results.push({ file, decision })
        continue
      }

      try {
        let mediaItem: MediaLibraryItem | null = null
        if (decision.type === 'movie') {
          const movie = await tmdb.searchMovie(decision.title, decision.year ? String(decision.year) : undefined, undefined)
          if (movie) {
            mediaItem = decorateAIScrapeItem(movieToMediaItem(movie, [toDriveFileItem(file)], folderName, undefined, file.path.substring(0, file.path.lastIndexOf('/')) || '', now), { ok: true, decision, provider: cfg.providerName })
          }
        } else if (decision.type === 'tv') {
          const tv = await tmdb.searchTV(decision.title, decision.season || 1, decision.year ? String(decision.year) : undefined, undefined)
          if (tv) {
            mediaItem = tvToMediaItem(tv, decision, [toDriveFileItem(file)], folderName, undefined, file.path.substring(0, file.path.lastIndexOf('/')) || '', now)
            if (mediaItem) mediaItem = decorateAIScrapeItem(mediaItem, { ok: true, decision, provider: cfg.providerName })
          }
        }
        results.push({ file, decision, mediaItem })
      } catch (e: any) {
        results.push({ file, decision, error: e?.message || 'TMDB 查询失败' })
      }
    }
    return results
  } catch (error: any) {
    const msg = error?.message || String(error)
    DebugLog.mSaveWarning('[BatchAIScrape] failed ' + msg)
    return files.map(f => ({ file: f, error: msg }))
  }
}

async function scrapeSingleMediaWithBoxPlayerCloud(input: MediaAIScrapeInput, timeoutMs: number): Promise<MediaAIScrapeDecision> {
  const file = input.file || input.item?.driveFiles?.[0]
  if (!file) throw new Error('没有可供刮削的视频文件')
  const results = await withTimeout(scrapeMediaWithBoxPlayerCloud([{
    id: file.id || '0',
    filename: file.name,
    folderHint: input.folderName || input.item?.folderPath || input.folderContext?.join(' | ') || '',
    pathHint: file.path
  }]), timeoutMs)
  const normalized = normalizeCloudMediaScrapeDecision(results[0])
  if (!normalized.decision) throw new Error(normalized.reason || 'AI 未返回有效刮削决策')
  DebugLog.mSaveLog('info', '[MediaAIScrape] cloud result ' + JSON.stringify(results[0]), undefined)
  return normalized.decision
}

async function scrapeBatchMediaWithBoxPlayerCloud(files: BatchScrapeFileInfo[], folderName: string): Promise<Array<MediaAIScrapeDecision | null>> {
  const decisions: Array<MediaAIScrapeDecision | null> = Array(files.length).fill(null)
  const chunks = Array.from({ length: Math.ceil(files.length / 10) }, (_, index) => files.slice(index * 10, index * 10 + 10))
  for (const [chunkIndex, chunk] of chunks.entries()) {
    const results = await withTimeout(scrapeMediaWithBoxPlayerCloud(chunk.map((file, index) => ({
      id: String(chunkIndex * 10 + index),
      filename: file.name,
      folderHint: folderName,
      pathHint: file.path
    }))), 60000)
    for (const result of results) {
      const byId = Number(result.id)
      const index = Number.isInteger(byId) ? byId : result.idx
      const normalized = normalizeCloudMediaScrapeDecision(result)
      if (typeof index === 'number' && Number.isInteger(index) && index >= 0 && index < files.length && normalized.usable && normalized.decision) decisions[index] = normalized.decision
    }
  }
  return decisions
}

function normalizeCloudMediaScrapeDecision(result: BoxPlayerCloudMediaScrapeResult | undefined): { usable: boolean, decision?: MediaAIScrapeDecision, reason?: string } {
  if (!result) return { usable: false, reason: 'AI 未返回刮削结果' }
  return normalizeMediaAIScrapeDecision({
    ...result,
    tmdbId: result.tmdb_id,
    allowOverwrite: true,
    reason: ''
  })
}

function toDriveFileItem(f: BatchScrapeFileInfo): DriveFileItem {
  if (f.driveFile) return f.driveFile
  return { id: '', name: f.name, path: f.path, userId: '', driveId: '', driveServerId: '', fileSize: f.fileSize || 0 }
}
