import type { MediaAcquisitionRunView } from '@shared/types/mediaAcquisition'
import Config from '../../config'
import { createPanHubFetch, discoverPanHubSources, searchPanHubStream } from '../../utils/panHubSearch'
import { createMediaAcquisitionCandidateInput } from './shareExecutor'
import { addMediaAcquisitionCandidate, addMediaAcquisitionEvent, beginMediaAcquisitionSearch } from './client'
import { getMediaAcquisitionCapability } from './capabilities'
import { isMediaAcquisitionCandidateSupported, scoreMediaAcquisitionCandidate } from './candidatePolicy'
import { buildMediaAcquisitionSearchKeywords, keywordReferencesTitle, stripQualitySubtitleTokens } from './searchGuard'

const API_BASE = `${Config.BOXPLAYER_API_URL.replace(/\/+$/, '')}/api`
// Keep the same snapshot visibility ceiling as mediary-scout. The Agent sees a
// bare-title pre-warm first, then decides whether a distinct search is needed.
const MAX_INITIAL_CANDIDATES = 120
const MAX_ADDITIONAL_CANDIDATES = 120

function supportsCandidate(run: MediaAcquisitionRunView, candidate: NonNullable<ReturnType<typeof createMediaAcquisitionCandidateInput>>): boolean {
  const capability = getMediaAcquisitionCapability(run.target.targetPlatform)
  // Keep the raw snapshot faithful to mediary-scout: provider payload is only
  // filtered by whether the target drive can execute it.  Title/year/episode
  // text is evidence for the Agent to judge, never a pre-Agent recall filter.
  return !!capability && isMediaAcquisitionCandidateSupported(run.target.targetPlatform, capability, candidate)
}

export async function searchAdditionalMediaAcquisitionCandidates(run: MediaAcquisitionRunView, keyword: string, reason = 'Agent 追加搜索'): Promise<number> {
  const normalizedKeyword = stripQualitySubtitleTokens(keyword)
  const titles = [run.target.title, ...(run.target.alternativeTitles || [])]
  if (!titles.some(title => keywordReferencesTitle(normalizedKeyword, title))) throw new Error('追加搜索关键词必须包含目标标题或标题别名')
  const fetchImpl = createPanHubFetch(window.Electron?.ipcRenderer?.invoke?.bind(window.Electron.ipcRenderer))
  const sources = await discoverPanHubSources(API_BASE, fetchImpl)
  await addMediaAcquisitionEvent(run.id, 'info', 'search', `Agent 追加搜索资源：${normalizedKeyword}`, { tool: 'searchResources', keyword: normalizedKeyword, reason, state: 'started' })
  const result = await searchPanHubStream({
    apiBase: API_BASE,
    keyword: normalizedKeyword,
    plugins: sources.plugins,
    channels: sources.channels,
    concurrency: 4,
    pluginTimeoutMs: 5000,
    fetchImpl,
    ipcRenderer: window.Electron?.ipcRenderer
  })
  const seen = new Set<string>()
  const candidates = Object.values(result.merged).flat().flatMap(link => {
    const candidate = createMediaAcquisitionCandidateInput(link.url, link.password, link.note || '')
    if (!candidate || seen.has(candidate.locator) || !supportsCandidate(run, candidate)) return []
    seen.add(candidate.locator)
    return [candidate]
  }).sort((left, right) => scoreMediaAcquisitionCandidate(run.target, right) - scoreMediaAcquisitionCandidate(run.target, left))
  let added = 0
  for (const candidate of candidates) {
    await addMediaAcquisitionCandidate(run.id, candidate)
    added++
    if (added >= MAX_ADDITIONAL_CANDIDATES) break
  }
  await addMediaAcquisitionEvent(run.id, added ? 'info' : 'warning', 'search', `追加搜索「${normalizedKeyword}」返回 ${result.total} 个资源，新增兼容候选 ${added} 个。`, { tool: 'searchResources', keyword: normalizedKeyword, total: result.total, added, state: 'completed' })
  return added
}

/** Search only candidates that the selected target drive can actually import. */
export async function searchMediaAcquisitionCandidates(run: MediaAcquisitionRunView): Promise<number> {
  await beginMediaAcquisitionSearch(run.id)
  const fetchImpl = createPanHubFetch(window.Electron?.ipcRenderer?.invoke?.bind(window.Electron.ipcRenderer))
  try {
    const sources = await discoverPanHubSources(API_BASE, fetchImpl)
    const seen = new Set<string>()
    const candidates: NonNullable<ReturnType<typeof createMediaAcquisitionCandidateInput>>[] = []
    // mediary-scout primes only the bare title. Aliases, season terms and
    // other variants are deliberate Agent actions and must not consume an
    // opaque automatic search batch before the Agent has seen the first result.
    const searchKeywords = buildMediaAcquisitionSearchKeywords(run.target).slice(0, 1)
    let successfulSearches = 0
    let failedSearches = 0
    let lastSearchError = ''
  await addMediaAcquisitionEvent(run.id, 'info', 'search', `Agent 将执行 ${searchKeywords.length} 轮资源搜索。`, { keywords: searchKeywords.map(item => item.keyword) })

    for (const plan of searchKeywords) {
      await addMediaAcquisitionEvent(run.id, 'info', 'search', `搜索资源快照：${plan.keyword}`, { tool: 'primeRawSnapshot', keyword: plan.keyword, reason: plan.reason, state: 'started' })
      let result: Awaited<ReturnType<typeof searchPanHubStream>>
      try {
        result = await searchPanHubStream({
          apiBase: API_BASE,
          keyword: plan.keyword,
          plugins: sources.plugins,
          channels: sources.channels,
          concurrency: 4,
          pluginTimeoutMs: 5000,
          fetchImpl,
          ipcRenderer: window.Electron?.ipcRenderer
        })
        successfulSearches++
      } catch (error: any) {
        failedSearches++
        lastSearchError = error?.message || '未知错误'
        await addMediaAcquisitionEvent(run.id, 'warning', 'search', `搜索快照「${plan.keyword}」请求失败，Agent 将继续尝试其它关键词：${lastSearchError}`, { keyword: plan.keyword, reason: plan.reason })
        continue
      }
      const links = Object.values(result.merged).flat()
      let compatibleCount = 0
      for (const link of links) {
        const candidate = createMediaAcquisitionCandidateInput(link.url, link.password, link.note || '')
        if (!candidate || seen.has(candidate.locator)) continue
        seen.add(candidate.locator)
        if (!supportsCandidate(run, candidate)) continue
        compatibleCount++
        candidates.push(candidate)
      }
      await addMediaAcquisitionEvent(run.id, compatibleCount ? 'info' : 'warning', 'search', `搜索快照「${plan.keyword}」返回 ${links.length} 个资源，兼容目标网盘 ${compatibleCount} 个。标题和年份将由 Agent 在已观察快照中判断。`, { tool: 'primeRawSnapshot', keyword: plan.keyword, total: links.length, compatible: compatibleCount, state: 'completed' })
      if (candidates.length >= MAX_INITIAL_CANDIDATES) break
    }
    if (!successfulSearches && failedSearches > 0) throw new Error(`全部资源搜索请求失败：${lastSearchError}`)

    candidates.sort((left, right) => scoreMediaAcquisitionCandidate(run.target, right) - scoreMediaAcquisitionCandidate(run.target, left))
    if (candidates.length) {
      if (failedSearches > 0) await addMediaAcquisitionEvent(run.id, 'warning', 'search', `部分搜索请求失败，但已获得 ${candidates.length} 个兼容候选，Agent 将继续执行。`, { failedSearches, successfulSearches })
      await addMediaAcquisitionEvent(run.id, 'info', 'search', `候选快照已生成：${candidates.length} 个兼容资源，已保留前 ${Math.min(candidates.length, MAX_INITIAL_CANDIDATES)} 个供 Agent 决策。`, {
        candidates: candidates.slice(0, MAX_INITIAL_CANDIDATES).map(candidate => ({
          kind: candidate.kind,
          sourcePlatform: candidate.sourcePlatform,
          title: candidate.title,
          detail: candidate.detail,
          score: scoreMediaAcquisitionCandidate(run.target, candidate)
        }))
      })
    }
    let added = 0
    for (const candidate of candidates) {
      await addMediaAcquisitionCandidate(run.id, candidate)
      added++
      if (added >= MAX_INITIAL_CANDIDATES) return added
    }
    return added
  } catch (error) {
    throw error
  }
}
