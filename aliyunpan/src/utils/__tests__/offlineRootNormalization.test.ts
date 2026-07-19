import { afterEach, describe, expect, it, vi } from 'vitest'

const { getUserToken } = vi.hoisted(() => ({ getUserToken: vi.fn(() => ({ access_token: 'token' })) }))
vi.mock('../../user/userdal', () => ({ default: { GetUserToken: getUserToken } }))

import { apiCloud123OfflineCreate } from '../../cloud123/offline'
import { apiPikPakOfflineCreate } from '../../pikpak/offline'

describe('offline root normalization', () => {
  afterEach(() => vi.restoreAllMocks())

  it('does not send the synthetic PikPak drive id as parent_id', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(JSON.stringify({ task: { id: 'task-1' }, file: { id: 'file-1' } }), { status: 200 }))
    await apiPikPakOfflineCreate('pikpak_user', 'magnet:?xt=urn:btih:test', 'Demo', 'pikpak')
    const body = JSON.parse(String((fetchMock.mock.calls[0][1] as RequestInit).body))
    expect(body.parent_id).toBeUndefined()
    expect(body.folder_type).toBe('DOWNLOAD')
  })

  it('does not send the synthetic 123 drive id as dirID', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(JSON.stringify({ code: 0, data: { taskID: 12 } }), { status: 200 }))
    await apiCloud123OfflineCreate('cloud123_user', 'https://example.com/demo.mp4', 'Demo', 'cloud123')
    const body = JSON.parse(String((fetchMock.mock.calls[0][1] as RequestInit).body))
    expect(body.dirID).toBeUndefined()
  })
})
