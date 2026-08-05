import { afterEach, describe, expect, it, vi } from 'vitest'

const { guangyaRequest } = vi.hoisted(() => ({ guangyaRequest: vi.fn() }))

vi.mock('../dirfilelist', () => ({
  guangyaApiParentId: (value: string) => value,
  guangyaRequest
}))

import { apiGuangyaTrashBatch } from '../filecmd'

afterEach(() => vi.resetAllMocks())

describe('Guangya file commands', () => {
  it('moves folder ids to the recycle bin through delete_file', async () => {
    guangyaRequest.mockResolvedValueOnce({ success: true })

    await expect(apiGuangyaTrashBatch('user', ['folder-1'])).resolves.toEqual(['folder-1'])
    expect(guangyaRequest).toHaveBeenCalledWith('user', '/nd.bizuserres.s/v1/file/delete_file', { fileIds: ['folder-1'] })
  })
})
