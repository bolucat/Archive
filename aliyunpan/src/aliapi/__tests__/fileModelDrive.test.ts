import { describe, expect, it } from 'vitest'
import { resolveAliFileDriveId } from '../fileModelDrive'

describe('resolveAliFileDriveId', () => {
  it('uses the directory request drive when a nested item omits drive_id', () => {
    expect(resolveAliFileDriveId(undefined, '1822729720')).toBe('1822729720')
    expect(resolveAliFileDriveId('', '1822729720')).toBe('1822729720')
  })

  it('keeps an explicit item drive id', () => {
    expect(resolveAliFileDriveId('explicit-drive', 'request-drive')).toBe('explicit-drive')
  })
})
