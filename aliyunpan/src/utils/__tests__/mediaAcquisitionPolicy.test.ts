import { describe, expect, it } from 'vitest'
import { getMediaAcquisitionCapability, normalizeMediaAcquisitionRootFolder, supportsMediaAcquisition } from '../../services/mediaAcquisition/capabilities'
import { assessMediaAcquisitionEpisodeCoverage, canTryNextMediaAcquisitionCandidate, isMediaAcquisitionCandidateDiscoverable, isMediaAcquisitionCandidateEligible, isMediaAcquisitionCandidateGapFocused, isMediaAcquisitionCandidateSupported, isMediaAcquisitionCandidateTitleMatched, isMediaAcquisitionCandidateTransparent, isMediaAcquisitionCandidateYearCompatible, isSystemicMediaAcquisitionFailure, isTransientMediaAcquisitionFailure, mediaAcquisitionCandidateCoveragePlan, pickMediaAcquisitionFallbackCandidate, scoreMediaAcquisitionCandidate } from '../../services/mediaAcquisition/candidatePolicy'
import { buildHistoricalGapPlans, selectHistoricalScanBatch } from '../../services/mediaAcquisition/historicalPolicy'
import { buildMediaAcquisitionLeafPath, isMediaAcquisitionMatchingSidecar, isMediaAcquisitionPrimaryVideoName } from '../../services/mediaAcquisition/organizerPolicy'
import { extractMediaAcquisitionEpisodeNumber, findMediaAcquisitionDuplicateEpisodes, selectRequestedEpisodeFiles } from '../../services/mediaAcquisition/duplicatePolicy'
import { newMediaAcquisitionFiles } from '../../services/mediaAcquisition/snapshotDiff'
import { buildSidecarSubtitleName, buildSubtitleSearchQuery, isConfirmedNonDomesticMediaOrigin, pickSubtitleReferenceVideo } from '../../services/mediaAcquisition/subtitleNaming'
import { buildMediaAcquisitionSearchKeywords } from '../../services/mediaAcquisition/searchGuard'
import { isMediaAcquisitionMovieUnreleased, mediaAcquisitionReleaseAt } from '@shared/mediaAcquisitionReleaseGate'
import type { MediaLibraryFolder, MediaLibraryItem } from '../../types/media'

describe('media acquisition provider policy', () => {
  it('reserves only movies with a known future release date', () => {
    const now = new Date('2026-07-18T04:00:00.000Z')
    expect(isMediaAcquisitionMovieUnreleased('2026-07-19', now)).toBe(true)
    expect(isMediaAcquisitionMovieUnreleased('2026-07-18', now)).toBe(false)
    expect(isMediaAcquisitionMovieUnreleased(undefined, now)).toBe(false)
    expect(mediaAcquisitionReleaseAt('2026-07-19')).toBe(Date.parse('2026-07-19T00:00:00+08:00'))
  })
  it('exposes Aliyun share import without offline download', () => {
    expect(getMediaAcquisitionCapability('aliyun')).toMatchObject({ shareImport: true, magnetOfflineDownload: false, externalUrlOfflineDownload: false })
  })

  it('allows 123 cloud drive HTTP offline tasks but rejects magnets', () => {
    const capability = getMediaAcquisitionCapability('cloud123')!
    expect(capability).toMatchObject({ shareImport: false, magnetOfflineDownload: false, externalUrlOfflineDownload: true })
    expect(supportsMediaAcquisition(capability)).toBe(true)
    expect(isMediaAcquisitionCandidateSupported('cloud123', capability, { kind: 'http', sourcePlatform: 'http', title: 'small.mp4' })).toBe(true)
    expect(isMediaAcquisitionCandidateSupported('cloud123', capability, { kind: 'magnet', sourcePlatform: 'magnet', title: 'small.mp4' })).toBe(false)
  })

  it('keeps 115 share-only fallback unavailable while retaining OpenAPI offline imports', () => {
    const capability = getMediaAcquisitionCapability('115')!
    expect(capability).toMatchObject({ shareImport: false, magnetOfflineDownload: true, externalUrlOfflineDownload: true })
    expect(isMediaAcquisitionCandidateSupported('115', capability, { kind: 'share', sourcePlatform: '115', title: 'Demo' })).toBe(false)
    expect(isMediaAcquisitionCandidateSupported('115', capability, { kind: 'magnet', sourcePlatform: 'magnet', title: 'Demo' })).toBe(true)
  })

  it('chooses the highest-scoring compatible 115 offline candidate when Agent selection is missing', () => {
    const capability = getMediaAcquisitionCapability('115')!
    const target = { targetPlatform: '115', title: '阿甘正传', year: 1994, preferredQuality: '1080p' }
    const candidate = pickMediaAcquisitionFallbackCandidate(target, capability, [
      { id: 'share', status: 'pending', kind: 'share', sourcePlatform: '115', title: '阿甘正传 1994 1080P' },
      { id: 'low', status: 'pending', kind: 'magnet', sourcePlatform: 'magnet', title: '阿甘正传 1994 720P' },
      { id: 'best', status: 'pending', kind: 'http', sourcePlatform: 'http', title: '阿甘正传 1994 1080P WEB-DL' }
    ])
    expect(candidate?.id).toBe('best')
  })


  it('gives share and magnet candidates the same base score', () => {
    const capability = getMediaAcquisitionCapability('guangya')!
    const share = { kind: 'share' as const, sourcePlatform: 'guangya', title: 'Demo 2160p 简中' }
    const magnet = { kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'Demo 2160p 简中' }
    const target = { year: 2026, preferredQuality: '2160p', preferredLanguage: 'zh-CN' }
    expect(isMediaAcquisitionCandidateSupported('guangya', capability, share)).toBe(true)
    expect(scoreMediaAcquisitionCandidate(target, share)).toBe(scoreMediaAcquisitionCandidate(target, magnet))
    expect(scoreMediaAcquisitionCandidate(target, magnet)).toBeGreaterThan(scoreMediaAcquisitionCandidate(target, { ...share, title: 'Demo 720p' }))
  })

  it('applies exact quality preference after recall without putting it in the query', () => {
    const candidate = (title: string) => ({ kind: 'magnet' as const, sourcePlatform: 'magnet', title })
    expect(scoreMediaAcquisitionCandidate({ preferredQuality: '2160p' }, candidate('Movie 4K UHD WEB-DL'))).toBeGreaterThan(scoreMediaAcquisitionCandidate({ preferredQuality: '2160p' }, candidate('Movie 1080P WEB-DL')))
    expect(scoreMediaAcquisitionCandidate({ preferredQuality: '1080p' }, candidate('Movie 1080P BluRay'))).toBeGreaterThan(scoreMediaAcquisitionCandidate({ preferredQuality: '1080p' }, candidate('Movie 2160P UHD REMUX')))
  })

  it('recognizes transparent candidates before considering opaque packages', () => {
    expect(isMediaAcquisitionCandidateTransparent({ kind: 'share', sourcePlatform: 'quark', title: 'Movie.2024.2160p.WEB-DL.HEVC.Atmos [18.6GB]' })).toBe(true)
    expect(isMediaAcquisitionCandidateTransparent({ kind: 'share', sourcePlatform: 'quark', title: '电影全集 高清合集' })).toBe(false)
  })

  it('applies every configured subtitle language to candidate ranking', () => {
    const candidate = (title: string) => ({ kind: 'share' as const, sourcePlatform: 'guangya', title })
    expect(scoreMediaAcquisitionCandidate({ fetchSubtitles: true, preferredLanguage: 'zh-Hant' }, candidate('Movie 繁中 CHT'))).toBeGreaterThan(scoreMediaAcquisitionCandidate({ fetchSubtitles: true, preferredLanguage: 'zh-Hant' }, candidate('Movie English')))
    expect(scoreMediaAcquisitionCandidate({ fetchSubtitles: true, preferredLanguage: 'en' }, candidate('Movie English ENG'))).toBeGreaterThan(scoreMediaAcquisitionCandidate({ fetchSubtitles: true, preferredLanguage: 'en' }, candidate('Movie 日语')))
    expect(scoreMediaAcquisitionCandidate({ fetchSubtitles: true, preferredLanguage: 'ja' }, candidate('Movie 日语 JPN'))).toBeGreaterThan(scoreMediaAcquisitionCandidate({ fetchSubtitles: true, preferredLanguage: 'ja' }, candidate('Movie 韩语')))
    expect(scoreMediaAcquisitionCandidate({ fetchSubtitles: true, preferredLanguage: 'ko' }, candidate('Movie 韩语 KOR'))).toBeGreaterThan(scoreMediaAcquisitionCandidate({ fetchSubtitles: true, preferredLanguage: 'ko' }, candidate('Movie English')))
  })

  it('enables automatic subtitles only for a confirmed non-Chinese origin', () => {
    expect(isConfirmedNonDomesticMediaOrigin(['US'])).toBe(true)
    expect(isConfirmedNonDomesticMediaOrigin(['CN'])).toBe(false)
    expect(isConfirmedNonDomesticMediaOrigin([])).toBe(false)
  })

  it('distinguishes dead candidates from account-wide transfer blocks', () => {
    expect(isSystemicMediaAcquisitionFailure('分享链接已失效')).toBe(false)
    expect(isSystemicMediaAcquisitionFailure('云下载配额不足，请升级 VIP')).toBe(true)
    expect(isSystemicMediaAcquisitionFailure('夸克网盘容量不足，还需 153.68GB')).toBe(true)
    expect(isSystemicMediaAcquisitionFailure('capacity limit[{0}]')).toBe(true)
    expect(isSystemicMediaAcquisitionFailure('access token expired, please login again')).toBe(true)
    expect(isSystemicMediaAcquisitionFailure('夸克网盘未登录或 cookie 无效')).toBe(true)
    expect(isSystemicMediaAcquisitionFailure('429 Too Many Requests')).toBe(true)
    expect(isSystemicMediaAcquisitionFailure('请求过于频繁，请稍后再试')).toBe(true)
  })

  it('retries only transport failures on the same candidate', () => {
    expect(isTransientMediaAcquisitionFailure('fetch failed: socket hang up')).toBe(true)
    expect(isTransientMediaAcquisitionFailure('503 Service Temporarily Unavailable')).toBe(true)
    expect(isTransientMediaAcquisitionFailure('分享不存在')).toBe(false)
    expect(isTransientMediaAcquisitionFailure('提取码错误')).toBe(false)
  })

  it('continues only with candidates supported by the target drive', () => {
    const candidates = [
      { id: 'share', kind: 'share' as const, sourcePlatform: 'pikpak', title: 'share', status: 'selected' as const },
      { id: 'magnet', kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'magnet', status: 'rejected' as const }
    ]
    expect(canTryNextMediaAcquisitionCandidate({ targetPlatform: 'pikpak' }, candidates, 'share', '分享链接已失效')).toBe(true)
    expect(canTryNextMediaAcquisitionCandidate({ targetPlatform: 'pikpak' }, candidates, 'share', '云下载配额不足')).toBe(false)
    expect(canTryNextMediaAcquisitionCandidate({ targetPlatform: 'pikpak' }, candidates, 'share', '夸克网盘容量不足，还需 153.68GB')).toBe(false)
    expect(canTryNextMediaAcquisitionCandidate({ targetPlatform: 'aliyun' }, candidates, 'share', '分享链接已失效')).toBe(false)
  })

  it('continues through ranked eligible candidates after a dead share', () => {
    const candidates = [
      { id: 'first', kind: 'share' as const, sourcePlatform: 'pikpak', title: 'Demo 1080p', status: 'failed' as const },
      { id: 'second', kind: 'share' as const, sourcePlatform: 'pikpak', title: 'Demo 1080p', status: 'selected' as const },
      { id: 'third', kind: 'share' as const, sourcePlatform: 'pikpak', title: 'Demo 1080p', status: 'rejected' as const }
    ]
    expect(canTryNextMediaAcquisitionCandidate({ targetPlatform: 'pikpak' }, candidates, 'second', '分享不存在')).toBe(true)
  })

  it('requires the primary title or a TMDB alias before a candidate is eligible', () => {
    expect(isMediaAcquisitionCandidateTitleMatched({ title: '黑暗骑士', alternativeTitles: ['The Dark Knight'] }, { kind: 'share', sourcePlatform: '115', title: 'The Dark Knight 1080P' })).toBe(true)
    expect(isMediaAcquisitionCandidateTitleMatched({ title: '黑暗骑士', alternativeTitles: ['The Dark Knight'] }, { kind: 'share', sourcePlatform: '115', title: '蝙蝠侠家族 1080P' })).toBe(false)
    expect(isMediaAcquisitionCandidateTitleMatched({ title: '黑暗骑士' }, { kind: 'share', sourcePlatform: '115', title: '' })).toBe(false)
  })

  it('rejects candidates with an explicit mismatched release year', () => {
    expect(isMediaAcquisitionCandidateYearCompatible({ year: 2008 }, { kind: 'share', sourcePlatform: '115', title: 'The Dark Knight 2008 1080P' })).toBe(true)
    expect(isMediaAcquisitionCandidateYearCompatible({ year: 2008 }, { kind: 'share', sourcePlatform: '115', title: 'The Dark Knight 2012 1080P' })).toBe(false)
    expect(isMediaAcquisitionCandidateYearCompatible({ year: 2008 }, { kind: 'share', sourcePlatform: '115', title: 'The Dark Knight 1080P' })).toBe(true)
  })

  it('uses aliases for recall and keeps movie years out of search keywords', () => {
    const keywords = buildMediaAcquisitionSearchKeywords({ mediaType: 'movie', title: '黑暗骑士', alternativeTitles: ['The Dark Knight'], year: 2008 })
    expect(keywords.map(item => item.keyword)).toEqual(['黑暗骑士', 'The Dark Knight'])
  })

  it('uses season and year fallback keywords for episodic media', () => {
    const keywords = buildMediaAcquisitionSearchKeywords({ mediaType: 'tv', title: '权力的游戏', alternativeTitles: ['Game of Thrones'], year: 2011, seasonNumber: 2, missingEpisodes: [4] })
    expect(keywords.map(item => item.keyword)).toEqual(['权力的游戏', 'Game of Thrones', '权力的游戏 S02E04', '权力的游戏 2011'])
  })

  it('ignores stale season coverage metadata for movie candidates', () => {
    const target = { mediaType: 'movie' as const, seasonNumber: 1, missingEpisodes: [1, 2] }
    const candidate = { kind: 'share' as const, sourcePlatform: 'quark', title: '千与千寻 2001 1080p' }
    expect(buildMediaAcquisitionSearchKeywords({ ...target, title: '千与千寻', year: 2001 }).map(item => item.keyword)).toEqual(['千与千寻'])
    expect(assessMediaAcquisitionEpisodeCoverage(target, candidate)).toEqual({ covers: true, matchedEpisodes: [], reason: '电影无需缺集覆盖判断' })
    expect(mediaAcquisitionCandidateCoveragePlan(target, [{ id: 'movie', ...candidate }])).toEqual({ covered: true, candidateIds: [], remaining: [] })
    expect(scoreMediaAcquisitionCandidate(target, candidate)).toBe(scoreMediaAcquisitionCandidate({ mediaType: 'movie' }, candidate))
  })

  it('keeps one cross-season task searchable and eligible one season at a time', () => {
    const target = { targetPlatform: '115', seasonTargets: [{ seasonNumber: 1, missingEpisodes: [4] }, { seasonNumber: 2, missingEpisodes: [1] }] }
    const capability = getMediaAcquisitionCapability('115')!
    const seasonOne = { kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'Demo.S01E04.1080p' }
    const seasonTwo = { kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'Demo.S02E01.1080p' }
    const keywords = buildMediaAcquisitionSearchKeywords({ mediaType: 'tv', title: 'Demo', ...target })
    expect(keywords.map(item => item.keyword)).toEqual(['Demo', 'Demo S01E04', 'Demo S02E01'])
    expect(assessMediaAcquisitionEpisodeCoverage(target, seasonOne).covers).toBe(true)
    expect(assessMediaAcquisitionEpisodeCoverage(target, seasonTwo).covers).toBe(true)
    expect(isMediaAcquisitionCandidateEligible(target, capability, seasonOne)).toBe(true)
  })

  it('builds the smallest ordered candidate set that covers every remaining season', () => {
    const target = { seasonTargets: [{ seasonNumber: 1, missingEpisodes: [4] }, { seasonNumber: 2, missingEpisodes: [1] }] }
    const plan = mediaAcquisitionCandidateCoveragePlan(target, [
      { id: 's1', kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'Demo.S01E04.1080p' },
      { id: 's2', kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'Demo.S02E01.1080p' },
      { id: 'redundant', kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'Demo.S01E04.1080p' }
    ])
    expect(plan).toEqual({ covered: true, candidateIds: ['s1', 's2'], remaining: [] })
  })

  it('unions partial candidates for the same season instead of discarding them', () => {
    const target = { seasonTargets: [{ seasonNumber: 1, missingEpisodes: [1, 2] }] }
    const plan = mediaAcquisitionCandidateCoveragePlan(target, [
      { id: 'e1', kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'Demo.S01E01.1080p' },
      { id: 'e2', kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'Demo.S01E02.1080p' }
    ])
    expect(plan).toEqual({ covered: true, candidateIds: ['e1', 'e2'], remaining: [] })
  })

  it('requires explicit episode coverage for missing and patrol tasks', () => {
    const capability = getMediaAcquisitionCapability('115')!
    const target = { targetPlatform: '115', seasonNumber: 1, missingEpisodes: [4, 5] }
    const exact = { kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'Demo.S01E04-E05.1080p' }
    const stale = { kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'Demo 第一季 更新至03集 1080p' }
    const vague = { kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'Demo 1080p 中字' }
    expect(assessMediaAcquisitionEpisodeCoverage(target, exact)).toMatchObject({ covers: true, matchedEpisodes: [4, 5] })
    expect(assessMediaAcquisitionEpisodeCoverage(target, stale)).toMatchObject({ covers: false, reason: expect.stringContaining('E3') })
    expect(isMediaAcquisitionCandidateEligible(target, capability, exact)).toBe(true)
    expect(isMediaAcquisitionCandidateEligible(target, capability, vague)).toBe(false)
    expect(isMediaAcquisitionCandidateDiscoverable(target, capability, vague)).toBe(true)
  })

  it('requires an explicit matching season for later-season gap fills', () => {
    const target = { seasonNumber: 2, missingEpisodes: [4] }
    expect(assessMediaAcquisitionEpisodeCoverage(target, { kind: 'share', sourcePlatform: '115', title: 'Demo E04 1080p' })).toMatchObject({ covers: false, reason: '目标为第 2 季，候选未明确标明季数' })
    expect(assessMediaAcquisitionEpisodeCoverage(target, { kind: 'share', sourcePlatform: '115', title: 'Demo 第二季 第4集 1080p' })).toMatchObject({ covers: true })
    expect(assessMediaAcquisitionEpisodeCoverage(target, { kind: 'share', sourcePlatform: '115', title: 'Demo S01E04 1080p' })).toMatchObject({ covers: false, reason: '候选标明第 1 季，不匹配目标第 2 季' })
  })

  it('prefers an exact missing-episode candidate over a complete-season package', () => {
    const target = { seasonNumber: 1, missingEpisodes: [4, 5] }
    expect(isMediaAcquisitionCandidateGapFocused(target, { kind: 'share', sourcePlatform: '115', title: 'Demo.S01E04.E05.1080p [2.4GB]' })).toBe(true)
    expect(isMediaAcquisitionCandidateGapFocused(target, { kind: 'share', sourcePlatform: '115', title: 'Demo S01 全集 2160p [32GB]' })).toBe(false)
  })

  it('falls back only to candidates that still cover requested missing episodes', () => {
    const target = { targetPlatform: '115', seasonNumber: 1, missingEpisodes: [6] }
    const candidates = [
      { id: 'current', kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'Demo.S01E06.1080p', status: 'selected' as const },
      { id: 'wrong', kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'Demo.S01E01-E03.1080p', status: 'pending' as const }
    ]
    expect(canTryNextMediaAcquisitionCandidate(target, candidates, 'current', '离线任务失败')).toBe(false)
    candidates.push({ id: 'pack', kind: 'magnet' as const, sourcePlatform: 'magnet', title: 'Demo 第一季 全集 1080p', status: 'pending' as const })
    expect(canTryNextMediaAcquisitionCandidate(target, candidates, 'current', '离线任务失败')).toBe(true)
  })

  it('normalizes provider drive ids into real root folder ids', () => {
    expect(normalizeMediaAcquisitionRootFolder('115', 'drive115')).toBe('drive115_root')
    expect(normalizeMediaAcquisitionRootFolder('pikpak', 'pikpak')).toBe('pikpak_root')
    expect(normalizeMediaAcquisitionRootFolder('quark', 'quark')).toBe('quark_root')
    expect(normalizeMediaAcquisitionRootFolder('guangya', 'guangya')).toBe('guangya_root')
    expect(normalizeMediaAcquisitionRootFolder('cloud123', 'cloud123')).toBe('cloud_root')
  })

  it('detects newly landed target files from a pre-transfer snapshot', () => {
    const before = [{ id: 'old', name: 'old.mkv', path: '/old.mkv', size: 1024 }]
    const after = [
      before[0],
      { id: 'new-zero', name: 'new-zero.mkv', path: '/new-zero.mkv', size: 0 },
      { id: 'new-good', name: 'new-good.mkv', path: '/new-good.mkv', size: 2048 }
    ]
    expect(newMediaAcquisitionFiles(before, after)).toEqual(after.slice(1))
  })

  it('builds subtitle queries and sidecar names from the largest landed video', () => {
    const files = [
      { id: 'small', name: 'Demo.S01E01.sample.mkv', path: '/Demo.S01E01.sample.mkv', size: 1024 },
      { id: 'main', name: 'Demo.S01E01.2026.1080p.WEB-DL.mkv', path: '/Demo.S01E01.2026.1080p.WEB-DL.mkv', size: 2048 }
    ]
    expect(pickSubtitleReferenceVideo(files)?.id).toBe('main')
    expect(buildSubtitleSearchQuery({ title: 'Demo', year: 2026 } as any, files)).toBe('Demo S01E01 2026 1080p WEB-DL')
    expect(buildSidecarSubtitleName(files[1].name, 'subtitle.ass')).toBe('Demo.S01E01.2026.1080p.WEB-DL.ass')
  })

  it('builds conservative media library leaf folders', () => {
    expect(buildMediaAcquisitionLeafPath({ mediaType: 'movie', title: 'Alien/Predator', year: 2026 })).toEqual(['Alien Predator (2026)'])
    expect(buildMediaAcquisitionLeafPath({ mediaType: 'tv', title: 'Demo', year: 2026, seasonNumber: 2 })).toEqual(['Demo (2026)', 'Season 2'])
    expect(buildMediaAcquisitionLeafPath({ mediaType: 'anime', title: 'Demo', seasonNumber: 1 })).toEqual(['Demo', 'Season 1'])
  })

  it('audits duplicate episodes and keeps the largest file', () => {
    const files = [
      { id: 'old-big', name: 'Demo.S01E04.1080p.mkv', path: '/Demo.S01E04.1080p.mkv', size: 1_200 },
      { id: 'new-small', name: 'Demo.S01E04.WEB-DL.mkv', path: '/Demo.S01E04.WEB-DL.mkv', size: 800 },
      { id: 'e5', name: 'Demo.S01E05.1080p.mkv', path: '/Demo.S01E05.1080p.mkv', size: 900 }
    ]
    const groups = findMediaAcquisitionDuplicateEpisodes({ mediaType: 'tv', seasonNumber: 1 }, files)
    expect(groups).toEqual([{ episode: 4, keep: files[0], deleteCandidates: [files[1]] }])
    expect(findMediaAcquisitionDuplicateEpisodes({ mediaType: 'movie', seasonNumber: 1 } as any, files)).toEqual([])
  })

  it('moves only requested episodes from a complete-season package', () => {
    const files = [
      { id: 'e1', name: 'Demo.S01E01.1080p.mkv', path: '/E01.mkv', size: 1000 },
      { id: 'e2', name: 'Demo.S01E02.1080p.mkv', path: '/E02.mkv', size: 1000 },
      { id: 'e3', name: 'Demo.S01E03.1080p.mkv', path: '/E03.mkv', size: 1000 }
    ]
    expect(selectRequestedEpisodeFiles({ mediaType: 'tv', seasonNumber: 1, missingEpisodes: [2] }, files)).toEqual([files[1]])
    expect(selectRequestedEpisodeFiles({ mediaType: 'tv', seasonNumber: 1, missingEpisodes: [4] }, files)).toEqual(files)
  })

  it('extracts common episode number formats for duplicate audit', () => {
    expect(extractMediaAcquisitionEpisodeNumber('Demo.S02E04.2160p.mkv', 2)).toBe(4)
    expect(extractMediaAcquisitionEpisodeNumber('Demo 2x04 HD.mkv', 2)).toBe(4)
    expect(extractMediaAcquisitionEpisodeNumber('Demo 第2季第4集.mkv', 2)).toBe(4)
  })

  it('keeps only primary videos and their matching sidecar subtitles', () => {
    expect(isMediaAcquisitionPrimaryVideoName('Demo.2026.1080p.mkv')).toBe(true)
    expect(isMediaAcquisitionPrimaryVideoName('Demo.2026.trailer.mkv')).toBe(false)
    expect(isMediaAcquisitionPrimaryVideoName('Demo.2026.花絮.mkv')).toBe(false)
    expect(isMediaAcquisitionMatchingSidecar('Demo.2026.1080p.ass', ['Demo.2026.1080p.mkv'])).toBe(true)
    expect(isMediaAcquisitionMatchingSidecar('Demo.2026.1080p.idx', ['Demo.2026.1080p.mkv'])).toBe(true)
    expect(isMediaAcquisitionMatchingSidecar('Demo.2026.1080p.sup', ['Demo.2026.1080p.mkv'])).toBe(true)
    expect(isMediaAcquisitionMatchingSidecar('Other.Movie.ass', ['Demo.2026.1080p.mkv'])).toBe(false)
  })
})

describe('historical media gap plans', () => {
  it('rotates bounded scans through the entire historical library', () => {
    const items = Array.from({ length: 5 }, (_, index) => index + 1)
    const first = selectHistoricalScanBatch(items, 0, 2)
    const second = selectHistoricalScanBatch(items, first.nextCursor, 2)
    const third = selectHistoricalScanBatch(items, second.nextCursor, 2)
    expect(first.items).toEqual([1, 2])
    expect(second.items).toEqual([3, 4])
    expect(third.items).toEqual([5, 1])
    expect(third.nextCursor).toBe(1)
  })

  it('creates one background acquisition per aired season gap', () => {
    const item: MediaLibraryItem = {
      id: 'tv-1', parentId: 'shows', folderId: 'source-1', type: 'tv', name: 'Demo', year: '2026', genres: [], tmdbId: 42, driveFiles: [], addedAt: new Date(),
      seasons: [{ id: 1, seasonNumber: 1, name: 'S1', episodeCount: 3, episodes: [{ id: 1, episodeNumber: 1, seasonNumber: 1, name: 'E1', driveFiles: [] }] }],
      expectedSeasons: [{ seasonNumber: 1, episodeCount: 3, latestAiredEpisode: 3 }]
    }
    const folder: MediaLibraryFolder = { id: 'source-1', fileId: 'drive115', name: 'Shows', path: '/', userId: '115_user', driveId: 'drive115', driveServerId: 'drive115', scanDate: new Date(), itemCount: 1 }
    const plans = buildHistoricalGapPlans(item, folder, '115', { mediaAcquisitionPreferredQuality: '1080p', mediaAcquisitionFetchSubtitles: true, mediaAcquisitionSubtitleLanguage: 'zh-CN' })
    expect(plans).toHaveLength(1)
    expect(plans[0]).toMatchObject({ kind: 'missing', seasonNumber: 1, missingEpisodes: [2, 3], targetPlatform: '115', targetParentFileId: 'drive115_root', trackingEnabled: true })
  })

  it('creates historical gap tasks for HTTP-only cloud drives', () => {
    const item: MediaLibraryItem = {
      id: 'tv-http', parentId: 'shows', folderId: 'source-http', type: 'tv', name: 'HTTP Demo', year: '2026', genres: [], tmdbId: 43, driveFiles: [], addedAt: new Date(),
      seasons: [], expectedSeasons: [{ seasonNumber: 1, episodeCount: 2, latestAiredEpisode: 2 }]
    }
    const folder: MediaLibraryFolder = { id: 'source-http', fileId: 'cloud123', name: 'Shows', path: '/', userId: '123_user', driveId: 'cloud123', driveServerId: 'cloud123', scanDate: new Date(), itemCount: 1 }
    const plans = buildHistoricalGapPlans(item, folder, 'cloud123', { mediaAcquisitionPreferredQuality: 'auto', mediaAcquisitionFetchSubtitles: false, mediaAcquisitionSubtitleLanguage: 'zh-CN' })
    expect(plans).toHaveLength(1)
    expect(plans[0]).toMatchObject({ seasonNumber: 1, missingEpisodes: [1, 2], targetPlatform: 'cloud123', targetParentFileId: 'cloud_root' })
  })
})
