import { afterEach, describe, expect, it, vi } from 'vitest'

const { guangyaRequest } = vi.hoisted(() => ({ guangyaRequest: vi.fn() }))

vi.mock('../dirfilelist', () => ({
  getGuangyaFileId: (item: { fileId?: string }) => item.fileId || '',
  guangyaApiParentId: (value: string) => value,
  isGuangyaDir: (item: { isDir?: boolean }) => item.isDir === true,
  guangyaRequest
}))

import { apiGuangyaMkdir, apiGuangyaTrashBatch } from '../filecmd'

afterEach(() => vi.resetAllMocks())

describe('Guangya file commands', () => {
  it('creates a directory and refuses to treat a same-named file as success', async () => {
    guangyaRequest.mockResolvedValueOnce({ data: { fileId: 'folder-1', isDir: true } })

    await expect(apiGuangyaMkdir('user', 'guangya_root', 'New Folder')).resolves.toEqual({ file_id: 'folder-1', error: '' })
    expect(guangyaRequest).toHaveBeenCalledWith('user', '/nd.bizuserres.s/v1/file/create_dir', {
      dirName: 'New Folder',
      parentId: 'guangya_root',
      failIfNameExist: true
    })

    guangyaRequest.mockResolvedValueOnce({ data: { fileId: 'file-1', isDir: false } })
    await expect(apiGuangyaMkdir('user', 'guangya_root', 'Existing File')).resolves.toEqual({ file_id: '', error: '同名资源已存在，未创建文件夹' })
  })

  it('moves folder ids to the recycle bin through delete_file', async () => {
    guangyaRequest.mockResolvedValueOnce({ success: true })

    await expect(apiGuangyaTrashBatch('user', ['folder-1'])).resolves.toEqual(['folder-1'])
    expect(guangyaRequest).toHaveBeenCalledWith('user', '/nd.bizuserres.s/v1/file/delete_file', { fileIds: ['folder-1'] })
  })
})
