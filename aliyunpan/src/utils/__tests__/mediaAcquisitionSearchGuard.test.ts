import { describe, expect, it } from 'vitest'
import { buildMediaAcquisitionSearchKeywords, keywordReferencesTitle, normalizeSearchKeyword, stripQualitySubtitleTokens } from '../../services/mediaAcquisition/searchGuard'

describe('media acquisition search guard', () => {
  it('strips quality and subtitle noise from search keywords', () => {
    expect(stripQualitySubtitleTokens('The Matrix 1999 2160p WEB-DL 中字 双语')).toBe('The Matrix 1999')
  })

  it('starts with a bare title prewarm keyword', () => {
    const plans = buildMediaAcquisitionSearchKeywords({ title: '鬼玩人', year: 1981 })
    expect(plans[0]).toEqual({ keyword: '鬼玩人', reason: '裸标题预热' })
    expect(plans.map(plan => plan.keyword)).toContain('鬼玩人 1981')
  })

  it('adds episode keywords for small missing episode sets', () => {
    const plans = buildMediaAcquisitionSearchKeywords({ title: 'Fallout', seasonNumber: 1, missingEpisodes: [2, 3] })
    expect(plans.map(plan => plan.keyword)).toContain('Fallout S01E02 S01E03')
  })

  it('rejects keywords that no longer reference the title', () => {
    expect(keywordReferencesTitle('2026 电影', '鬼玩人')).toBe(false)
    expect(keywordReferencesTitle('Evil Dead 1080p', 'Evil Dead')).toBe(true)
  })

  it('normalizes repeated search keywords without quality noise', () => {
    expect(normalizeSearchKeyword('The.Matrix.1999.1080p.中字')).toBe('the matrix 1999')
    expect(normalizeSearchKeyword('The Matrix 1999 WEB-DL 双语')).toBe('the matrix 1999')
  })
})
