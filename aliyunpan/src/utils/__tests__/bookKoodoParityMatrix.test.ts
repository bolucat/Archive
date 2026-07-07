import { describe, expect, it } from 'vitest'
import { KOODO_PARITY_FEATURES, getIncompleteKoodoFeatures } from '../bookKoodoParityMatrix'

describe('bookKoodoParityMatrix', () => {
  it('tracks every non-sync Koodo feature area with an owner file', () => {
    expect(KOODO_PARITY_FEATURES.length).toBeGreaterThan(30)

    for (const feature of KOODO_PARITY_FEATURES) {
      expect(feature.id).toMatch(/^[a-z0-9-]+$/)
      expect(feature.koodoSource.length).toBeGreaterThan(0)
      expect(['missing', 'partial', 'complete', 'excluded']).toContain(feature.status)

      if (feature.status === 'excluded') {
        expect(feature.reason).toMatch(/account|sync/i)
      } else {
        expect(feature.targetFiles.length).toBeGreaterThan(0)
      }
    }
  })

  it('keeps account and cloud sync explicitly excluded', () => {
    const excluded = KOODO_PARITY_FEATURES.filter((item) => item.status === 'excluded').map((item) => item.id)

    expect(excluded).toEqual(expect.arrayContaining([
      'account-login',
      'cloud-sync',
      'remote-backup-sync',
    ]))
  })

  it('reports actionable incomplete non-sync work', () => {
    const incompleteFeatures = getIncompleteKoodoFeatures()

    expect(incompleteFeatures.every((item) => item.status !== 'excluded')).toBe(true)
    expect(incompleteFeatures.map((item) => item.id)).toEqual(expect.arrayContaining([
      'manager-shell-visuals',
      'manager-header-buttons',
      'reader-panel-buttons',
      'reader-settings-complete',
    ]))
  })
})
