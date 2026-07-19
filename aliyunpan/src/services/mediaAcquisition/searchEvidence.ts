import type { MediaAcquisitionEvent } from '@shared/types/mediaAcquisition'
import { normalizeSearchKeyword } from './searchGuard'

export interface MediaAcquisitionSearchEvidence {
  providerKeywords: Set<string>
  agentKeywords: Set<string>
}

/** Persisted search evidence is the task-level source of truth across runner restarts. */
export function getMediaAcquisitionSearchEvidence(events: MediaAcquisitionEvent[]): MediaAcquisitionSearchEvidence {
  const providerKeywords = new Set<string>()
  const agentKeywords = new Set<string>()
  for (const event of events) {
    if (event.phase !== 'search') continue
    const data = event.data || {}
    if (data.state === 'started') continue
    const keyword = typeof data.keyword === 'string' ? data.keyword : keywordFromMessage(event.message)
    if (!keyword) continue
    const normalized = normalizeSearchKeyword(keyword)
    if (!normalized) continue
    providerKeywords.add(normalized)
    if (data.tool === 'searchResources' || /^Agent 追加搜索资源[：:]/.test(event.message)) agentKeywords.add(normalized)
  }
  return { providerKeywords, agentKeywords }
}

export function hasMediaAcquisitionProviderSearchEvidence(events: MediaAcquisitionEvent[]): boolean {
  return getMediaAcquisitionSearchEvidence(events).providerKeywords.size > 0
}

function keywordFromMessage(message: string): string {
  return /^搜索资源快照[：:]\s*(.+)$/.exec(message)?.[1] || /^Agent 追加搜索资源[：:]\s*(.+)$/.exec(message)?.[1] || /^追加搜索「(.+)」/.exec(message)?.[1] || ''
}
