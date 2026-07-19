import { describe, expect, it } from 'vitest'
import { getBoxPlayerMediaScrapeUrl, mapBoxPlayerCloudAIError } from '../boxplayerCloudAI'

describe('BoxPlayer Cloud AI error mapping', () => {
  it('routes media scraping through the BoxPlayer resource API', () => {
    expect(getBoxPlayerMediaScrapeUrl()).toBe('https://boxplayer-api-673444103572.europe-west1.run.app/v1/media-scrape')
  })

  it('shows a useful monthly credit message for a worker quota response', () => {
    expect(mapBoxPlayerCloudAIError('monthly_ai_credit_quota_exceeded')).toContain('本月内置 AI 额度已用完')
  })

  it('maps body-less 429 errors emitted by the PI streaming client', () => {
    expect(mapBoxPlayerCloudAIError('429 status code (no body)')).toContain('本月内置 AI 额度已用完')
  })

  it('maps the Cloudflare bridge 500 observed for a rejected cloud request', () => {
    expect(mapBoxPlayerCloudAIError('500 status code (no body)')).toContain('本月内置 AI 额度已用完')
  })
})
