import { describe, expect, it } from 'vitest'
import { getMediaAcquisitionSearchEvidence, hasMediaAcquisitionProviderSearchEvidence } from '../../services/mediaAcquisition/searchEvidence'

describe('media acquisition search evidence', () => {
  it('persists provider and agent search keywords across runner restarts', () => {
    const events: any[] = [
      { phase: 'search', message: '搜索资源快照：千与千寻', data: { keyword: '千与千寻' } },
      { phase: 'search', message: 'Agent 追加搜索资源：Spirited Away', data: { tool: 'searchResources', keyword: 'Spirited Away', state: 'started' } },
      { phase: 'search', message: '追加搜索「Spirited Away」返回 2 个资源', data: { tool: 'searchResources', keyword: 'Spirited Away', state: 'completed' } }
    ]
    const evidence = getMediaAcquisitionSearchEvidence(events)
    expect([...evidence.providerKeywords]).toEqual(['千与千寻', 'spirited away'])
    expect([...evidence.agentKeywords]).toEqual(['spirited away'])
    expect(hasMediaAcquisitionProviderSearchEvidence(events)).toBe(true)
  })

  it('does not accept an Agent no-coverage conclusion without provider evidence', () => {
    expect(hasMediaAcquisitionProviderSearchEvidence([{ phase: 'select', message: 'Agent 未找到可靠候选' } as any])).toBe(false)
  })
})
