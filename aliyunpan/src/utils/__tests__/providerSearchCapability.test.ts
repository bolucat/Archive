import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { supportsProviderSearch } from '../../services/agent/providerCapabilities'

describe('provider search capabilities', () => {
  it('only exposes search for providers with an implemented search operation', () => {
    expect(supportsProviderSearch('aliyun')).toBe(true)
    expect(supportsProviderSearch('cloud123')).toBe(true)
    expect(supportsProviderSearch('115')).toBe(true)
    expect(supportsProviderSearch('baidu')).toBe(true)
    expect(supportsProviderSearch('pikpak')).toBe(false)
    expect(supportsProviderSearch('139')).toBe(false)
    expect(supportsProviderSearch('189')).toBe(false)
    expect(supportsProviderSearch('webdav')).toBe(false)
  })

  it('uses the same capability for the drive tree and cross-drive search', () => {
    const root = resolve(__dirname, '../../..')
    const leftTree = readFileSync(resolve(root, 'src/pan/PanLeft.vue'), 'utf8')
    const globalSearch = readFileSync(resolve(root, 'src/utils/globalSearch.ts'), 'utf8')

    expect(leftTree).toContain("item.key === 'search' && !supportsSearch")
    expect(globalSearch).toContain('supportsProviderSearch(token.tokenfrom)')
  })
})
