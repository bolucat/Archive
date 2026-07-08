import DebugLog from './debuglog'
import message from './message'
import { generateAIText, resolveAIProviderConfig } from './bookAI'
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

export function canUseMediaAIScrape(options: { manual: boolean; isBYOK?: boolean }): { allowed: boolean, message?: string } {
  if (options.manual) return checkAndIncrement('mediaAIScrape', 1, { metered: false, isBYOK: options.isBYOK })
  if (!isPro() && !options.isBYOK) return { allowed: false, message: 'AI 影视刮削需购买 Pro 后使用' }
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
  if (!decision.title && !decision.tmdbId) return { usable: false, decision, reason: 'AI 未返回片名或 TMDB ID' }
  if (decision.confidence < MIN_CONFIDENCE) return { usable: false, decision, reason: `AI 置信度过低：${decision.confidence}` }
  if (decision.type === 'tv' && (!decision.season || !decision.episode)) return { usable: false, decision, reason: 'AI 未返回剧集季集信息' }
  return { usable: true, decision }
}

export async function scrapeMediaWithAI(input: MediaAIScrapeInput, options: { manual?: boolean, timeoutMs?: number } = {}): Promise<MediaAIScrapeResult> {
  const cfg = resolveAIProviderConfig()
  const isBYOK = !!cfg && cfg.providerName !== 'boxplayer-cloud'
  const gate = canUseMediaAIScrape({ manual: options.manual === true, isBYOK })
  if (!gate.allowed) return { ok: false, error: gate.message || 'AI 影视刮削不可用' }
  if (!cfg) return { ok: false, error: '请先配置 AI 模型' }

  try {
    DebugLog.mSaveLog('info', '[MediaAIScrape] input ' + JSON.stringify(buildInputLog(input)), undefined)
    const prompt = buildAIScrapePrompt(input)
    const output = await withTimeout(generateAIText(cfg, prompt, 900), options.timeoutMs || 20000)
    DebugLog.mSaveLog('info', '[MediaAIScrape] model output ' + output.slice(0, 2000), undefined)
    const decision = parseMediaAIScrapeDecision(output)
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
    const movie = await tmdb.searchMovie(decision.title, decision.year ? String(decision.year) : undefined, decision.tmdbId ? String(decision.tmdbId) : undefined, file.fileHash || file.contentHash, file.name)
    if (!movie) return null
    return decorateAIScrapeItem(movieToMediaItem(movie, files, folderName, folderId, folderPath, now), result, input.item)
  }

  if (decision.type === 'tv') {
    const tv = await tmdb.searchTV(decision.title, decision.season || 1, decision.year ? String(decision.year) : undefined, decision.tmdbId ? String(decision.tmdbId) : undefined, file.fileHash || file.contentHash, file.name)
    if (!tv) return null
    const item = tvToMediaItem(tv, decision, files, folderName, folderId, folderPath, now)
    return item ? decorateAIScrapeItem(item, result, input.item) : null
  }

  return null
}

export async function manualAIScrapeItem(item: MediaLibraryItem): Promise<MediaLibraryItem | null> {
  const result = await scrapeMediaWithAI({ item, currentMatch: mediaItemToCurrentMatch(item) }, { manual: true })
  if (!result.ok) {
    message.error(result.error || 'AI 重刮削失败')
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
    message.error(firstError || 'AI 重刮削未匹配到 TMDB 结果')
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

function buildAIScrapePrompt(input: MediaAIScrapeInput): string {
  const file = input.file || input.item?.driveFiles?.[0]
  return [
    '你是影视媒体库刮削 Agent。你只负责识别文件应该查询什么，不生成播放 URL，也不返回候选列表。',
    '只输出一个严格 JSON 对象，不要 markdown，不要解释。',
    'JSON schema: {"type":"movie|tv|unknown","title":"片名","year":2024,"season":1,"episode":1,"tmdbId":123,"confidence":0.0,"allowOverwrite":true,"reason":"短原因"}',
    '规则：电视剧必须给 season 和 episode；无法判断就 type=unknown；confidence 低于 0.65 时不要猜。',
    '',
    `文件名: ${file?.name || input.item?.name || ''}`,
    `路径: ${file?.path || ''}`,
    `父文件夹: ${input.folderName || input.item?.parentId || ''}`,
    `大小: ${file?.fileSize || 0}`,
    `hash: ${file?.fileHash || file?.contentHash || ''}`,
    `时长: ${file?.videoDuration || ''}`,
    `当前匹配: ${JSON.stringify(input.currentMatch || (input.item ? mediaItemToCurrentMatch(input.item) : {}))}`,
    `同目录上下文: ${(input.folderContext || []).slice(0, 12).join(' | ')}`
  ].join('\n')
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
  const isBYOK = !!cfg && cfg.providerName !== 'boxplayer-cloud'
  const gate = canUseMediaAIScrape({ manual: true, isBYOK })
  if (!gate.allowed) {
    return files.map(f => ({ file: f, error: gate.message || 'AI 影视刮削不可用' }))
  }

  if (!cfg) {
    return files.map(f => ({ file: f, error: '请先配置 AI 模型' }))
  }

  try {
    const prompt = buildBatchAIScrapePrompt(files, folderName)
    DebugLog.mSaveLog('info', '[BatchAIScrape] input ' + files.length + ' files, folder: ' + folderName, undefined)
    const output = await withTimeout(generateAIText(cfg, prompt, 900), 60000)
    DebugLog.mSaveLog('info', '[BatchAIScrape] model output ' + output.slice(0, 2000), undefined)
    const decisions = parseBatchAIScrapeOutput(output, files.length)
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
          const movie = await tmdb.searchMovie(decision.title, decision.year ? String(decision.year) : undefined, decision.tmdbId ? String(decision.tmdbId) : undefined)
          if (movie) {
            mediaItem = decorateAIScrapeItem(movieToMediaItem(movie, [toDriveFileItem(file)], folderName, undefined, file.path.substring(0, file.path.lastIndexOf('/')) || '', now), { ok: true, decision, provider: cfg.providerName })
          }
        } else if (decision.type === 'tv') {
          const tv = await tmdb.searchTV(decision.title, decision.season || 1, decision.year ? String(decision.year) : undefined, decision.tmdbId ? String(decision.tmdbId) : undefined)
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

function toDriveFileItem(f: BatchScrapeFileInfo): DriveFileItem {
  if (f.driveFile) return f.driveFile
  return { id: '', name: f.name, path: f.path, userId: '', driveId: '', driveServerId: '', fileSize: f.fileSize || 0 }
}

function buildBatchAIScrapePrompt(files: BatchScrapeFileInfo[], folderName: string): string {
  const fileList = files.map((f, i) => `${i + 1}. ${f.name}`).join('\n')
  return [
    '你是影视媒体库刮削 Agent。下面是同一个文件夹内的文件名列表，请逐一识别每部影视作品。',
    '只输出一个 JSON 数组，不要 markdown，不要解释。',
    `文件夹名: ${folderName}`,
    `文件总数: ${files.length}`,
    '',
    '文件列表:',
    fileList,
    '',
    '对每个文件，输出格式: {"index":1,"type":"movie|tv|unknown","title":"片名","year":2024,"season":1,"episode":1,"tmdbId":123,"confidence":0.0,"reason":"短原因"}',
    '规则:',
    '- index 对应文件编号',
    '- 电视剧必须给 season 和 episode',
    '- 无法判断就 type=unknown',
    '- confidence 低于 0.65 时不要猜',
    '- 如果多个文件明显是同一部剧的不同集，合并识别',
  ].join('\n')
}

function parseBatchAIScrapeOutput(text: string, fileCount: number): (MediaAIScrapeDecision | null)[] {
  const results: (MediaAIScrapeDecision | null)[] = new Array(fileCount).fill(null)
  try {
    const raw = extractJson(text)
    const parsed = JSON.parse(raw)
    const arr = Array.isArray(parsed) ? parsed : (parsed.results || parsed.items || [parsed])
    for (const item of arr) {
      if (!item || typeof item !== 'object') continue
      const idx = typeof item.index === 'number' ? item.index - 1 : -1
      const normalized = normalizeMediaAIScrapeDecision(item)
      if (normalized.usable && normalized.decision) {
        if (idx >= 0 && idx < fileCount) {
          results[idx] = normalized.decision
        } else {
          // try to match by title
          results[results.findIndex(r => r === null)] = normalized.decision
        }
      }
    }
  } catch { /* parse failure returns all null */ }
  return results
}
