import { afterEach, describe, expect, it, vi } from 'vitest'
import { deleteMediaServerItem } from '../deleteMedia'
import type { MediaServerConfig } from '../../types/mediaServer'
const config = (type: MediaServerConfig['type']) => ({ id: 'owning-server', name: type, type, baseUrl: 'https://server.invalid', accessToken: 'test-token', userId: 'user' } as MediaServerConfig)
afterEach(() => vi.unstubAllGlobals())
describe('media server deletion', () => {
  it.each(['emby', 'jellyfin', 'plex'] as const)('deletes only the selected %s item using its owning server', async type => {
    const request = vi.fn().mockResolvedValue({ ok: true, status: 204 })
    vi.stubGlobal('fetch', request)
    await deleteMediaServerItem(config(type), 'item/1')
    expect(request).toHaveBeenCalledOnce()
    expect(request.mock.calls[0][0]).toBe('https://server.invalid' + (type === 'plex' ? '/library/metadata/' : '/Items/') + 'item%2F1')
    expect(request.mock.calls[0][1].method).toBe('DELETE')
    expect(JSON.stringify(request.mock.calls[0][1].headers)).toContain('test-token')
  })
  it('propagates permission failures instead of reporting success', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 403 }))
    await expect(deleteMediaServerItem(config('jellyfin'), 'item')).rejects.toThrow('403')
  })
  it('rejects empty IDs without a request', async () => {
    const request = vi.fn(); vi.stubGlobal('fetch', request)
    await expect(deleteMediaServerItem(config('emby'), ' ')).rejects.toThrow()
    expect(request).not.toHaveBeenCalled()
  })
})
