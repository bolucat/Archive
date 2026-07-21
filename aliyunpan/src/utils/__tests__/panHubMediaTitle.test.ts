import { describe, expect, it } from 'vitest'
import { getPanHubSearchTitle } from '../panHubMediaTitle'

describe('getPanHubSearchTitle', () => {
  it('removes Douban ratings and ranks from the search term', () => {
    expect(getPanHubSearchTitle('【9.7】肖申克的救赎')).toBe('肖申克的救赎')
    expect(getPanHubSearchTitle('#1 【8.5】千与千寻')).toBe('千与千寻')
  })

  it('keeps ordinary media titles unchanged', () => {
    expect(getPanHubSearchTitle('沙丘：第二部')).toBe('沙丘：第二部')
  })
})
