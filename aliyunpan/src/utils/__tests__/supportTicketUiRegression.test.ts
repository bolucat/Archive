import { describe, expect, it } from 'vitest'

describe('support ticket UI regressions', () => {
  it('keeps the pan split divider visible in both light and dark themes', async () => {
    const source = await import('../../layout/MySplit.vue?raw')

    expect(source.default).toContain('.splitline::before')
    expect(source.default).toContain("body[arco-theme='dark'] .splitline::before")
    expect(source.default).toContain('rgba(31, 35, 41, 0.3)')
    expect(source.default).toContain('rgba(255, 255, 255, 0.28)')
  })
})
