import type { CreateMediaAcquisitionCandidateInput, MediaAcquisitionCandidate, MediaAcquisitionTarget } from '@shared/types/mediaAcquisition'
import type { MediaAcquisitionCapability } from './capabilities'
import { getMediaAcquisitionCapability, normalizeMediaAcquisitionPlatform } from './capabilities'

type CandidateLike = Pick<CreateMediaAcquisitionCandidateInput, 'kind' | 'sourcePlatform' | 'title' | 'detail'> | Pick<MediaAcquisitionCandidate, 'kind' | 'sourcePlatform' | 'title' | 'detail'>
type RetryCandidateLike = CandidateLike & Pick<MediaAcquisitionCandidate, 'id' | 'status'>
type ScoreTarget = Partial<Pick<MediaAcquisitionTarget, 'mediaType' | 'year' | 'seasonNumber' | 'missingEpisodes' | 'seasonTargets' | 'preferredQuality' | 'fetchSubtitles' | 'preferredLanguage' | 'title' | 'alternativeTitles'>>
type EligibilityTarget = ScoreTarget & Pick<MediaAcquisitionTarget, 'targetPlatform'>

const QUALITY_TOKENS: Array<{ tier: number; pattern: RegExp }> = [
  { tier: 4, pattern: /(?:2160p|\b4k\b|uhd)/i },
  { tier: 3, pattern: /(?:1080[pi]?|\bfhd\b)/i },
  { tier: 2, pattern: /(?:720[pi]?|\bhd\b)/i },
  { tier: 1, pattern: /(?:480[pi]?|\bsd\b)/i }
]

const LANGUAGE_TOKENS: Record<string, RegExp> = {
  'zh-CN': /(?:简体|简中|简繁|中文字幕|中文|中英|双语|\bchs\b|zh-cn|zh-hans)/i,
  'zh-Hant': /(?:繁体|繁中|简繁|中文字幕|中文|\bcht\b|zh-tw|zh-hant)/i,
  en: /(?:英文|英语|english|中英|双语|\beng\b|en-us)/i,
  ja: /(?:日文|日语|日語|japanese|\bjpn\b|\bja(?:-jp)?\b)/i,
  ko: /(?:韩文|韓文|韩语|韓語|korean|\bkor\b|\bko(?:-kr)?\b)/i
}

const TRANSPARENT_SIZE = /(?:\[|\(|\b)\s*\d+(?:\.\d+)?\s*(?:tb|gb|mb)\b/i
const TRANSPARENT_TECHNICAL_DETAIL = /(?:2160p|1080[pi]?|720[pi]?|\b4k\b|uhd|web[ ._-]?dl|webrip|blu[ ._-]?ray|bdrip|remux|\bx26[45]\b|\bh\.?26[45]\b|hevc|av1|dts|truehd|atmos|aac|flac)/i

export function isMediaAcquisitionCandidateSupported(targetPlatform: string, capability: MediaAcquisitionCapability, candidate: CandidateLike): boolean {
  if (candidate.kind === 'magnet') return capability.magnetOfflineDownload
  if (candidate.kind === 'http') return capability.externalUrlOfflineDownload
  return capability.shareImport && normalizeMediaAcquisitionPlatform(candidate.sourcePlatform) === normalizeMediaAcquisitionPlatform(targetPlatform)
}

export function assessMediaAcquisitionEpisodeCoverage(target: ScoreTarget, candidate: CandidateLike): { covers: boolean; matchedEpisodes: number[]; reason: string } {
  if (target.mediaType === 'movie') return { covers: true, matchedEpisodes: [], reason: '电影无需缺集覆盖判断' }
  const seasonTargets = target.seasonTargets?.filter(item => Number.isInteger(item.seasonNumber) && item.seasonNumber > 0) || []
  if (seasonTargets.length > 1) {
    const matches = seasonTargets.map(item => ({ seasonNumber: item.seasonNumber, coverage: assessMediaAcquisitionEpisodeCoverage({ ...target, seasonTargets: undefined, seasonNumber: item.seasonNumber, missingEpisodes: item.missingEpisodes }, candidate) })).filter(item => item.coverage.covers)
    if (matches.length) return { covers: true, matchedEpisodes: matches.flatMap(item => item.coverage.matchedEpisodes), reason: `候选覆盖跨季计划中的 S${String(matches[0].seasonNumber).padStart(2, '0')}：${matches[0].coverage.reason}` }
    return { covers: false, matchedEpisodes: [], reason: `候选未明确覆盖跨季计划（${seasonTargets.map(item => `S${String(item.seasonNumber).padStart(2, '0')}`).join('、')}）` }
  }
  const missingEpisodes = [...new Set(target.missingEpisodes || [])].filter(number => Number.isInteger(number) && number > 0).sort((a, b) => a - b)
  if (!missingEpisodes.length) return { covers: true, matchedEpisodes: [], reason: '无需缺集覆盖判断' }
  const text = `${candidate.title} ${candidate.detail || ''}`.normalize('NFKC')
  const seasonNumber = target.seasonNumber || 1
  const seasonMatch = readCandidateSeason(text)
  if (seasonNumber > 1 && !seasonMatch) return { covers: false, matchedEpisodes: [], reason: `目标为第 ${seasonNumber} 季，候选未明确标明季数` }
  if (seasonMatch && seasonMatch !== seasonNumber) return { covers: false, matchedEpisodes: [], reason: `候选标明第 ${seasonMatch} 季，不匹配目标第 ${seasonNumber} 季` }

  const maxMissing = Math.max(...missingEpisodes)
  const upToMatch = text.match(/(?:更新至|更至|至|up\s*to)\s*(?:E|EP|第)?\s*(\d{1,3})\s*(?:集|话|話|episodes?)?/i)
  if (upToMatch && Number(upToMatch[1]) < maxMissing) return { covers: false, matchedEpisodes: [], reason: `候选仅更新至 E${Number(upToMatch[1])}` }

  const covered = new Set<number>()
  const addRange = (start: number, end: number) => {
    const low = Math.min(start, end)
    const high = Math.max(start, end)
    for (const episode of missingEpisodes) if (episode >= low && episode <= high) covered.add(episode)
  }
  for (const match of text.matchAll(/\bS0?\d{1,2}[ ._\-]*E(?:P)?[ ._\-]*(\d{1,3})(?:\s*[-~至到]\s*(?:E|EP)?\s*(\d{1,3}))?/gi)) {
    const start = Number(match[1])
    const end = Number(match[2] || match[1])
    addRange(start, end)
  }
  for (const match of text.matchAll(/(?:第|\bE(?:P)?\s*)?(\d{1,3})\s*[-~至到]\s*(?:第|\bE(?:P)?\s*)?(\d{1,3})\s*(?:集|话|話|episodes?\b)/gi)) addRange(Number(match[1]), Number(match[2]))
  for (const match of text.matchAll(/(?:\bE(?:P)?[ ._\-]*|第\s*)(\d{1,3})\s*(?:集|话|話)?/gi)) {
    const episode = Number(match[1])
    if (missingEpisodes.includes(episode)) covered.add(episode)
  }

  const matchedEpisodes = [...covered].sort((a, b) => a - b)
  if (missingEpisodes.every(episode => covered.has(episode))) return { covers: true, matchedEpisodes, reason: `候选标题覆盖缺集 E${matchedEpisodes.join('、E')}` }

  const totalMatch = text.match(/全\s*(\d{1,3})\s*(?:集|话|話)|(?:全集|全季|complete)/i)
  if (totalMatch && (!totalMatch[1] || Number(totalMatch[1]) >= maxMissing)) return { covers: true, matchedEpisodes: missingEpisodes, reason: '候选标题标明全集/全季，覆盖缺集' }

  return { covers: false, matchedEpisodes, reason: matchedEpisodes.length ? `候选仅覆盖 E${matchedEpisodes.join('、E')}，仍缺 E${missingEpisodes.filter(episode => !covered.has(episode)).join('、E')}` : '候选标题无法确认覆盖缺集' }
}

export function mediaAcquisitionCandidateCoveragePlan(target: ScoreTarget, candidates: Array<CandidateLike & { id: string }>): { covered: boolean; candidateIds: string[]; remaining: Array<{ seasonNumber: number; missingEpisodes: number[] }> } {
  if (target.mediaType === 'movie') return { covered: true, candidateIds: [], remaining: [] }
  const targets = target.seasonTargets?.length ? target.seasonTargets : target.missingEpisodes?.length ? [{ seasonNumber: target.seasonNumber || 1, missingEpisodes: target.missingEpisodes }] : []
  const remaining = new Map(targets.map(item => [item.seasonNumber, new Set(item.missingEpisodes.filter(episode => Number.isInteger(episode) && episode > 0))]))
  const selected: string[] = []
  for (const candidate of candidates) {
    const matched = targets.flatMap(item => {
      const coverage = assessMediaAcquisitionEpisodeCoverage({ ...target, seasonTargets: undefined, seasonNumber: item.seasonNumber, missingEpisodes: [...(remaining.get(item.seasonNumber) || [])] }, candidate)
      // A candidate can contribute only part of the remaining season. Keep the
      // concrete episode evidence so several candidates may form one complete
      // cross-candidate plan instead of requiring each to cover a whole season.
      return coverage.matchedEpisodes.length ? [{ seasonNumber: item.seasonNumber, episodes: coverage.matchedEpisodes }] : []
    })
    if (!matched.length) continue
    matched.forEach(item => item.episodes.forEach(episode => remaining.get(item.seasonNumber)?.delete(episode)))
    selected.push(candidate.id)
    if ([...remaining.values()].every(episodes => !episodes.size)) break
  }
  return { covered: [...remaining.values()].every(episodes => !episodes.size), candidateIds: selected, remaining: [...remaining].map(([seasonNumber, episodes]) => ({ seasonNumber, missingEpisodes: [...episodes].sort((left, right) => left - right) })).filter(item => item.missingEpisodes.length) }
}

function readCandidateSeason(text: string): number | undefined {
  const match = text.match(/(?:\bS\s*0?(\d{1,2})(?=\b|[ ._\-]*E)|\bSEASON\s*0?(\d{1,2})\b|第\s*(\d{1,2})\s*季|第\s*([一二三四五六七八九十]+)\s*季)/i)
  if (!match) return undefined
  const numeric = Number(match[1] || match[2] || match[3])
  if (Number.isInteger(numeric) && numeric > 0) return numeric
  return chineseSeasonNumber(match[4])
}

function chineseSeasonNumber(value: string | undefined): number | undefined {
  if (!value) return undefined
  const numbers: Record<string, number> = { 一: 1, 二: 2, 三: 3, 四: 4, 五: 5, 六: 6, 七: 7, 八: 8, 九: 9, 十: 10 }
  if (value === '十') return 10
  if (value.length === 1) return numbers[value]
  if (value.startsWith('十')) return 10 + (numbers[value.slice(1)] || 0)
  if (value.endsWith('十')) return (numbers[value[0]] || 0) * 10
  if (value.length === 2 && value[1] in numbers) return (numbers[value[0]] || 0) * 10 + numbers[value[1]]
  return undefined
}

export function isMediaAcquisitionCandidateTitleMatched(target: Pick<ScoreTarget, 'title' | 'alternativeTitles'>, candidate: CandidateLike): boolean {
  const text = `${candidate.title} ${candidate.detail || ''}`.normalize('NFKC').toLowerCase().replace(/[^\p{L}\p{N}]+/gu, '')
  const titles = [target.title, ...(target.alternativeTitles || [])]
    .filter((title): title is string => typeof title === 'string')
    .map(title => title.normalize('NFKC').toLowerCase().replace(/[^\p{L}\p{N}]+/gu, ''))
    .filter(title => title.length >= 2)
  return !titles.length || titles.some(title => text.includes(title))
}

export function isMediaAcquisitionCandidateYearCompatible(target: Pick<ScoreTarget, 'year'>, candidate: CandidateLike): boolean {
  if (!target.year) return true
  const years = [...`${candidate.title} ${candidate.detail || ''}`.matchAll(/\b((?:19|20)\d{2})\b/g)].map(match => Number(match[1]))
  return !years.length || years.includes(target.year)
}

export function isMediaAcquisitionCandidateEligible(target: EligibilityTarget, capability: MediaAcquisitionCapability, candidate: CandidateLike): boolean {
  return isMediaAcquisitionCandidateSupported(target.targetPlatform, capability, candidate) && isMediaAcquisitionCandidateTitleMatched(target, candidate) && isMediaAcquisitionCandidateYearCompatible(target, candidate) && assessMediaAcquisitionEpisodeCoverage(target, candidate).covers
}

export function pickMediaAcquisitionFallbackCandidate(target: EligibilityTarget, capability: MediaAcquisitionCapability, candidates: RetryCandidateLike[]): RetryCandidateLike | undefined {
  return candidates
    .filter(candidate => candidate.status === 'pending' && isMediaAcquisitionCandidateEligible(target, capability, candidate))
    .sort((left, right) => scoreMediaAcquisitionCandidate(target, right) - scoreMediaAcquisitionCandidate(target, left))[0]
}

/**
 * Keep recall deliberately broader than execution eligibility.  The sandbox
 * must be able to inspect an otherwise plausible package before deciding
 * whether its actual contents cover a gap; title metadata alone is not a
 * reliable episode inventory.
 */
export function isMediaAcquisitionCandidateDiscoverable(target: EligibilityTarget, capability: MediaAcquisitionCapability, candidate: CandidateLike): boolean {
  return isMediaAcquisitionCandidateSupported(target.targetPlatform, capability, candidate) && isMediaAcquisitionCandidateTitleMatched(target, candidate) && isMediaAcquisitionCandidateYearCompatible(target, candidate)
}

/** A transparent candidate carries enough title evidence to avoid blind package transfers. */
export function isMediaAcquisitionCandidateTransparent(candidate: CandidateLike): boolean {
  const text = `${candidate.title} ${candidate.detail || ''}`
  return TRANSPARENT_SIZE.test(text) && TRANSPARENT_TECHNICAL_DETAIL.test(text)
}

/** Type 3 gap filling must prefer a candidate explicitly scoped to the missing episodes. */
export function isMediaAcquisitionCandidateGapFocused(target: Pick<ScoreTarget, 'seasonNumber' | 'missingEpisodes'>, candidate: CandidateLike): boolean {
  const missing = [...new Set(target.missingEpisodes || [])].filter(number => Number.isInteger(number) && number > 0).sort((a, b) => a - b)
  if (!missing.length || /(?:全集|全季|complete|season\s*pack)/i.test(`${candidate.title} ${candidate.detail || ''}`)) return false
  const text = `${candidate.title} ${candidate.detail || ''}`.normalize('NFKC')
  const season = target.seasonNumber || 1
  const episodeToken = (episode: number) => new RegExp(`(?:S0?${season}[ ._\\-]*E(?:P)?[ ._\\-]*0?${episode}(?!\\d)|第\\s*0?${episode}\\s*(?:集|话|話)?|\\bE(?:P)?[ ._\\-]*0?${episode}(?!\\d))`, 'i')
  if (!missing.every(episode => episodeToken(episode).test(text))) return false
  return assessMediaAcquisitionEpisodeCoverage(target, candidate).covers
}

export function scoreMediaAcquisitionCandidate(target: ScoreTarget, candidate: CandidateLike): number {
  const text = `${candidate.title} ${candidate.detail || ''}`.toLowerCase()
  let score = 0
  if (target.year && text.includes(String(target.year))) score += 8

  const qualityTier = QUALITY_TOKENS.find(item => item.pattern.test(text))?.tier || 0
  const preferredTier = ({ '2160p': 4, '1080p': 3, '720p': 2, '480p': 1 } as Record<string, number>)[target.preferredQuality || '']
  if (preferredTier && qualityTier) score += Math.max(0, 40 - Math.abs(preferredTier - qualityTier) * 14)
  else if (!preferredTier && qualityTier) score += qualityTier * 4
  if (/(?:web-?dl|blu-?ray|bdrip|webrip)/i.test(text)) score += 4
  if (/(?:枪版|camrip|\bcam\b|telesync)/i.test(text)) score -= 30
  if (/(?:蓝光原盘|\bbdmv\b|\biso\b)/i.test(text)) score -= 18

  if (target.fetchSubtitles !== false && target.preferredLanguage && target.preferredLanguage !== 'auto' && LANGUAGE_TOKENS[target.preferredLanguage]?.test(text)) score += 16

  const seasonTargets = target.mediaType === 'movie' ? [] : target.seasonTargets?.length ? target.seasonTargets : target.missingEpisodes?.length ? [{ seasonNumber: target.seasonNumber || 1, missingEpisodes: target.missingEpisodes }] : []
  if (seasonTargets.length) {
    const matched = seasonTargets.reduce((count, target) => count + target.missingEpisodes.filter(episode => new RegExp(`S0?${target.seasonNumber}[ ._\\-]*E(?:P)?[ ._\\-]*0?${episode}(?!\\d)`, 'i').test(text)).length, 0)
    score += matched * 25
    if (/(?:全集|全\s*\d+\s*[集话話]|complete|全季)/i.test(text)) score += 20
  }
  return score
}

export function isSystemicMediaAcquisitionFailure(message: string | undefined): boolean {
  const text = String(message || '')
  return /(?:云下载|离线下载).*(?:配额|额度|次数).*(?:不足|用完|耗尽|限制)|(?:配额|额度).*(?:不足|用完|耗尽)|(?:容量|空间|存储空间).*(?:不足|已满|超限|限制)|\b(?:capacity|storage)\b.*(?:exceed|exhaust|insufficient|limit|full)|\bquota\b.*(?:exceed|exhaust|insufficient|limit)|(?:请求|操作).*(?:过于频繁|频繁|太快)|限流|稍后再试|\brate[ _-]?limit(?:ed)?\b|\btoo many requests\b|(?:^|\D)429(?:\D|$)|(?:未|重新|请先)?(?:登录|登陆).*(?:超时|失效|过期|无效)?|(?:cookie|session).*(?:无效|失效|过期|invalid|expired)|鉴权|认证失败|未授权|\bunauthori[sz]ed\b|\bforbidden\b|invalid[_\s-]*(?:access[_\s-]*)?token|(?:access[_\s-]*)?token.*(?:expired|invalid)|\bvip\b|会员(?:专享|限制|不足)/i.test(text)
}

/** Only transport failures should retry the same candidate. Resource failures move on. */
export function isTransientMediaAcquisitionFailure(message: string | undefined): boolean {
  return /(?:\b(?:econnreset|etimedout|econnrefused|enotfound|eai_again|epipe)\b|socket(?:\s+hang\s+up|\s+disconnected)|fetch failed|network(?:\s+error)?|网络(?:异常|错误|不稳定)?|timeout|超时|\b50[234]\b|服务(?:暂时)?不可用|temporarily unavailable)/i.test(String(message || ''))
}

export function canTryNextMediaAcquisitionCandidate(target: EligibilityTarget, candidates: RetryCandidateLike[], currentCandidateId: string, message?: string): boolean {
  if (isSystemicMediaAcquisitionFailure(message)) return false
  const capability = getMediaAcquisitionCapability(target.targetPlatform)
  if (!capability) return false
  // Automatic fallback remains stricter than sandbox recall. A candidate that
  // has no episode evidence may be inspected by the Agent, but must not be
  // submitted automatically after another transfer has already failed.
  return candidates.some(candidate => candidate.id !== currentCandidateId && (candidate.status === 'pending' || candidate.status === 'rejected') && isMediaAcquisitionCandidateEligible(target, capability, candidate))
}
