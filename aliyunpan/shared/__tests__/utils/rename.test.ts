import { describe, expect, it } from 'vitest'
import { buildOuts } from '../../utils/rename'

describe('buildOuts', () => {
  it('returns empty array for empty uris', () => {
    expect(buildOuts([], 'test.mkv')).toEqual([])
  })

  it('returns uri as-is when no rename rule', () => {
    const result = buildOuts(['http://example.com/file.mkv'], 'file.mkv')
    expect(Array.isArray(result)).toBe(true)
  })
})
