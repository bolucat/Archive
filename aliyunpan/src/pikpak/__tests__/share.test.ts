import { afterEach, describe, expect, it, vi } from 'vitest'

vi.mock('../../user/userdal', () => ({
  default: { GetUserToken: vi.fn(() => ({ access_token: 'access-token' })) }
}))

import { apiPikPakSaveShareFilesBatch, apiPikPakShareFileList, apiPikPakShareToken, parsePikPakShareLink } from '../share'

describe('PikPak share helpers', () => {
  afterEach(() => vi.restoreAllMocks())

  it('parses a PikPak share link and its pass code', () => {
    expect(parsePikPakShareLink('https://mypikpak.com/s/share-id 提取码: a1b2')).toEqual({ id: 'pikpak:share-id', pwd: 'a1b2' })
  })

  it('uses the pass-code token to load the shared root folder', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(JSON.stringify({
      files: [{ id: 'folder-1', kind: 'drive#folder', name: 'Movies', size: '0' }]
    }), { status: 200 }))

    const result = await apiPikPakShareFileList('pikpak:share-id', 'pass-token', 'root')

    expect(result.error).toBe('')
    expect(result.items[0]).toMatchObject({ file_id: 'folder-1', isDir: true, drive_id: 'pikpak' })
    expect(String(fetchMock.mock.calls[0][0])).toContain('share_id=share-id')
    expect(String(fetchMock.mock.calls[0][0])).toContain('pass_code_token=pass-token')
  })

  it('returns the access token from the share response', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(JSON.stringify({ pass_code_token: 'pass-token' }), { status: 200 }))
    await expect(apiPikPakShareToken('pikpak:share-id', 'a1b2')).resolves.toBe('pass-token')
  })

  it('loads all share pages', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(new Response(JSON.stringify({ files: [{ id: 'file-1', kind: 'drive#file', name: 'one.mkv' }], next_page_token: 'next' }), { status: 200 }))
      .mockResolvedValueOnce(new Response(JSON.stringify({ files: [{ id: 'file-2', kind: 'drive#file', name: 'two.mkv' }] }), { status: 200 }))

    const result = await apiPikPakShareFileList('pikpak:share-id', 'pass-token', 'root')

    expect(result.items.map(item => item.file_id)).toEqual(['file-1', 'file-2'])
    expect(String(fetchMock.mock.calls[1][0])).toContain('page_token=next')
  })

  it('restores shares to PikPak root even when the final target is a child folder', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(JSON.stringify({}), { status: 200 }))

    await expect(apiPikPakSaveShareFilesBatch('pikpak:share-id', 'pass-token', 'user-1', 'child-folder', ['file-1'])).resolves.toBe('success')
    expect(JSON.parse(String(fetchMock.mock.calls[0][1]?.body))).toEqual({ share_id: 'share-id', pass_code_token: 'pass-token', file_ids: ['file-1'] })
  })
})
