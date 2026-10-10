import { afterEach, describe, expect, it, vi } from 'vitest'
import { getMediaServerLibraryPagedItems } from '../contentGateway'
import { SERVER_FAVORITE_TYPES, serverFavoriteID, serverFavoriteType, serverFavoriteTypes } from '../favoriteCategories'
import type { MediaServerConfig } from '../../types/mediaServer'

const config: MediaServerConfig = {
  id: 'server', type: 'emby', name: 'My Emby', baseUrl: 'http://localhost:8096',
  accessToken: 'test-token', userId: 'user', deviceId: 'device', createdAt: 1, updatedAt: 1
}

afterEach(() => vi.unstubAllGlobals())

describe('server favorite categories', () => {
  it('keeps the existing favorites identifier separate and excludes unsupported providers', () => {
    expect(serverFavoriteType('favorites')).toBeUndefined()
    expect(serverFavoriteTypes({ ...config, type: 'plex' })).toEqual([])
    expect(serverFavoriteTypes(config)).toHaveLength(7)
  })

  for (const type of SERVER_FAVORITE_TYPES) {
    for (const provider of ['emby', 'jellyfin'] as const) {
      it(`${provider}: fetches only favorite ${type} items with pagination`, async () => {
        const fetchMock = vi.fn(async (_url: string) => ({ ok: true, json: async () => ({
          Items: [{ Id: 'item', Name: 'Favorite', Type: type, UserData: { IsFavorite: true } }],
          TotalRecordCount: 101
        }) }))
        vi.stubGlobal('fetch', fetchMock)
        const result = await getMediaServerLibraryPagedItems({ ...config, type: provider }, serverFavoriteID(type), 1)
        const url = new URL(String(fetchMock.mock.calls[0]?.[0]))
        expect(url.pathname).toBe(type === 'Person' ? '/Persons' : '/Users/user/Items')
        expect(url.searchParams.get('IsFavorite')).toBe('true')
        expect(url.searchParams.get('IncludeItemTypes')).toBe(type === 'Person' ? null : type)
        expect(url.searchParams.get('UserId')).toBe('user')
        expect(url.searchParams.get('StartIndex')).toBe('50')
        expect(result.items).toHaveLength(1)
        expect(result.hasNextPage).toBe(true)
      })
    }
  }

  it('opens collection contents without inheriting favorite or movie-only filters', async () => {
    const fetchMock = vi.fn(async (_url: string) => ({ ok: true, json: async () => ({ Items: [], TotalRecordCount: 0 }) }))
    vi.stubGlobal('fetch', fetchMock)
    await getMediaServerLibraryPagedItems(config, 'server-container:collection', 0)
    const url = new URL(String(fetchMock.mock.calls[0]?.[0]))
    expect(url.searchParams.get('ParentId')).toBe('collection')
    expect(url.searchParams.has('IsFavorite')).toBe(false)
    expect(url.searchParams.has('IncludeItemTypes')).toBe(false)
    expect(url.searchParams.get('Recursive')).toBe('false')
  })
})
