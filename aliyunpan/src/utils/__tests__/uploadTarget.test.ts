import { describe, expect, it } from 'vitest'
import { isValidDropUploadTarget } from '../uploadTarget'

describe('isValidDropUploadTarget', () => {
  it('allows Baidu variable-length fs_ids for desktop drag uploads', () => {
    expect(isValidDropUploadTarget('baidu_user', 'baidu', '38112063721')).toBe(true)
  })

  it('keeps the legacy Aliyun identifier guard', () => {
    expect(isValidDropUploadTarget('aliyun_user', '', '38112063721')).toBe(false)
    expect(isValidDropUploadTarget('aliyun_user', '', 'a'.repeat(40))).toBe(true)
    expect(isValidDropUploadTarget('legacy_aliyun_account', 'real_drive_id', 'a'.repeat(40))).toBe(true)
  })

  it('allows valid non-Aliyun folder IDs used by drag uploads', () => {
    expect(isValidDropUploadTarget('cloud123_user', 'cloud123', '123456')).toBe(true)
    expect(isValidDropUploadTarget('115_user', 'drive115', '987654')).toBe(true)
    expect(isValidDropUploadTarget('dropbox_user', 'dropbox', 'id:folder')).toBe(true)
    expect(isValidDropUploadTarget('onedrive_user', 'onedrive', '01ABCDEF')).toBe(true)
  })

  it('routes desktop drops through the current file-list drive and directory fallbacks', async () => {
    const modalSource = await import('../modal.ts?raw')
    const panRightSource = await import('../../pan/PanRight.vue?raw')
    const providerCommandSource = await import('../../drive/providerFileCmd.ts?raw')

    expect(modalSource.default).toContain('panTreeStore.drive_id || panFileStore.DriveID')
    expect(modalSource.default).toContain('file_id || panFileStore.DirID || panTreeStore.selectDir.file_id')
    expect(panRightSource.default).toContain('modalUpload(targetDirId, files)')
    expect(providerCommandSource.default).toContain('TreeStore.GetDir(fileStore.DriveID, parentFileId)')
  })
})
