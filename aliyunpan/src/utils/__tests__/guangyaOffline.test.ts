import { beforeEach, describe, expect, it, vi } from 'vitest'

const { guangyaRequest } = vi.hoisted(() => ({ guangyaRequest: vi.fn() }))
vi.mock('../../guangya/dirfilelist', () => ({
  guangyaApiParentId: (id?: string) => !id || id.includes('root') ? '' : id,
  guangyaRequest
}))

import { apiGuangyaOfflineCreate, apiGuangyaOfflineProcess } from '../../guangya/offline'

describe('Guangya offline workflow', () => {
  beforeEach(() => guangyaRequest.mockReset())

  it('uses the mediary-scout resolve, create and task-list contract', async () => {
    guangyaRequest
      .mockResolvedValueOnce({ data: { url: 'magnet:?xt=urn:btih:demo', btResInfo: { fileName: 'Demo', subfiles: [{ fileIndex: 0, fileName: 'Demo.mkv' }, { fileIndex: 1, fileName: 'readme.txt' }] } } })
      .mockResolvedValueOnce({ data: { taskId: 'task-1' } })
      .mockResolvedValueOnce({ data: { list: [{ taskId: 'task-1', status: 2, progress: 100, fileId: 'file-1' }] } })

    await expect(apiGuangyaOfflineCreate('guangya_user', 'magnet:?xt=urn:btih:demo', 'Fallback', 'guangya_root')).resolves.toEqual({ taskId: 'task-1', fileId: '', error: '' })
    expect(guangyaRequest).toHaveBeenNthCalledWith(1, 'guangya_user', '/cloudcollection/v1/resolve_res', { url: 'magnet:?xt=urn:btih:demo' })
    expect(guangyaRequest).toHaveBeenNthCalledWith(2, 'guangya_user', '/cloudcollection/v1/create_task', { url: 'magnet:?xt=urn:btih:demo', parentId: '', newName: 'Demo', fileIndexes: [0] })

    await expect(apiGuangyaOfflineProcess('guangya_user', 'task-1')).resolves.toEqual({ status: 2, process: 100, error: '' })
    expect(guangyaRequest).toHaveBeenNthCalledWith(3, 'guangya_user', '/cloudcollection/v1/list_task', { taskIds: ['task-1'] })
  })

  it('submits HTTP subtitle links directly without magnet resolution', async () => {
    guangyaRequest.mockResolvedValueOnce({ data: { taskId: 'subtitle-task' } })
    await expect(apiGuangyaOfflineCreate('guangya_user', 'https://example.com/subtitle.srt', 'Demo.zh-CN.srt', 'folder-1')).resolves.toEqual({ taskId: 'subtitle-task', fileId: '', error: '' })
    expect(guangyaRequest).toHaveBeenCalledTimes(1)
    expect(guangyaRequest).toHaveBeenCalledWith('guangya_user', '/cloudcollection/v1/create_task', { url: 'https://example.com/subtitle.srt', parentId: 'folder-1', newName: 'Demo.zh-CN.srt' })
  })
})
