import { describe, expect, it, vi } from 'vitest'

vi.mock('../../user/userdal', () => ({
  default: { GetUserToken: () => undefined }
}))

import { isWritableProviderDirectory, supportsCreateFolder, supportsLocalUpload, supportsShareImport } from '../../drive/providerFeatures'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

describe('provider write-directory guard', () => {
  it('rejects virtual collection directories', () => {
    expect(isWritableProviderDirectory('google_shared')).toBe(false)
    expect(isWritableProviderDirectory('google_shared_drives')).toBe(false)
    expect(isWritableProviderDirectory('search:movie')).toBe(false)
  })

  it('allows actual roots and folders', () => {
    expect(isWritableProviderDirectory('')).toBe(true)
    expect(isWritableProviderDirectory('google_root')).toBe(true)
    expect(isWritableProviderDirectory('folder-id')).toBe(true)
  })

  it('derives toolbar actions from each provider capability manifest', () => {
    expect(supportsCreateFolder('', 'guangya_root')).toBe(true)
    expect(supportsLocalUpload('', 'guangya_root')).toBe(true)
    expect(supportsShareImport('', 'guangya_root')).toBe(true)

    expect(supportsCreateFolder('', 'cloud139_root')).toBe(true)
    expect(supportsLocalUpload('', 'cloud139_root')).toBe(false)
    expect(supportsShareImport('', 'cloud139_root')).toBe(false)

    expect(supportsCreateFolder('', 'drive115_root')).toBe(true)
    expect(supportsLocalUpload('', 'drive115_root')).toBe(true)
    expect(supportsShareImport('', 'drive115_root')).toBe(false)
  })

  it('normalizes Google text-file uploads to the shared string result contract', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/google/adapter.ts'), 'utf8')

    expect(source).toContain("return result.error || 'success'")
  })
})
