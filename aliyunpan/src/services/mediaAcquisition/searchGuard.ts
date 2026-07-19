import type { MediaAcquisitionTarget } from '@shared/types/mediaAcquisition'

const QUALITY_AND_SUBTITLE_TOKENS = /(?:\b(?:4k|2160p|1080p|720p|480p|uhd|hdr|dv|remux|web-?dl|webrip|bluray|blu-?ray|bdrip|x265|x264|hevc|avc)\b|蓝光原盘|蓝光|中字|中文字幕|国语|国配|双语|字幕|内封|外挂)/gi
const SEARCH_SEPARATOR = /[\s._\-:：/\\()[\]【】]+/g
const MAX_SEARCH_KEYWORDS = 4

export interface MediaAcquisitionSearchKeyword {
  keyword: string
  reason: string
}

function normalizeText(text: string): string {
  return text.toLowerCase().replace(SEARCH_SEPARATOR, ' ').replace(/\s+/g, ' ').trim()
}

export function normalizeSearchKeyword(keyword: string): string {
  return normalizeText(stripQualitySubtitleTokens(keyword))
}

export function stripQualitySubtitleTokens(keyword: string): string {
  return keyword.replace(QUALITY_AND_SUBTITLE_TOKENS, ' ').replace(/\s+/g, ' ').trim()
}

export function keywordReferencesTitle(keyword: string, title: string): boolean {
  const normalizedKeyword = normalizeText(keyword)
  const normalizedTitle = normalizeText(title)
  if (!normalizedKeyword || !normalizedTitle) return false
  if (normalizedKeyword.includes(normalizedTitle)) return true
  const titleParts = normalizedTitle.split(' ').filter(part => part.length >= 2)
  if (!titleParts.length) return normalizedKeyword.includes(normalizedTitle)
  return titleParts.some(part => normalizedKeyword.includes(part))
}

function appendKeyword(plans: MediaAcquisitionSearchKeyword[], seen: Set<string>, keyword: string, reason: string, titles: string[]): void {
  const stripped = stripQualitySubtitleTokens(keyword)
  const normalized = normalizeText(stripped)
  if (!normalized || seen.has(normalized) || !titles.some(title => keywordReferencesTitle(stripped, title))) return
  seen.add(normalized)
  plans.push({ keyword: stripped, reason })
}

type SearchTarget = Pick<MediaAcquisitionTarget, 'title' | 'year' | 'seasonNumber' | 'missingEpisodes' | 'seasonTargets'> & Partial<Pick<MediaAcquisitionTarget, 'mediaType' | 'alternativeTitles'>>

export function buildMediaAcquisitionSearchKeywords(target: SearchTarget): MediaAcquisitionSearchKeyword[] {
  const plans: MediaAcquisitionSearchKeyword[] = []
  const seen = new Set<string>()
  const titles = [target.title, ...(target.alternativeTitles || [])].map(title => title.trim()).filter(Boolean)
  appendKeyword(plans, seen, target.title, '裸标题预热', titles)
  for (const title of titles.slice(1)) appendKeyword(plans, seen, title, '原始标题/别名', titles)

  const seasonTargets = target.seasonTargets?.length ? target.seasonTargets : target.seasonNumber ? [{ seasonNumber: target.seasonNumber, missingEpisodes: target.missingEpisodes || [] }] : []
  for (const seasonTarget of seasonTargets) {
    const seasonToken = `S${String(seasonTarget.seasonNumber).padStart(2, '0')}`
    const episodeToken = seasonTarget.missingEpisodes.length && seasonTarget.missingEpisodes.length <= 3 ? seasonTarget.missingEpisodes.map(episode => `${seasonToken}E${String(episode).padStart(2, '0')}`).join(' ') : seasonToken
    appendKeyword(plans, seen, `${target.title} ${episodeToken}`, seasonTarget.missingEpisodes.length ? '缺集关键词' : '季关键词', titles)
  }

  if (target.mediaType !== 'movie' && target.year) appendKeyword(plans, seen, `${target.title} ${target.year}`, '年份兜底', titles)
  return plans.slice(0, MAX_SEARCH_KEYWORDS)
}
