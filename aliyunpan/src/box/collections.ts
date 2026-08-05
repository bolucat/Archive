import { boxApiRequest, mapBoxItemToAliModel } from './dirfilelist'

type BoxCollection = { id?: string; type?: string; name?: string }

export const buildBoxRecentItemsPath = (limit = 100) => `/recent_items?${new URLSearchParams({ limit: String(limit) }).toString()}`
export const buildBoxCollectionsPath = () => '/collections'
export const buildBoxCollectionItemsPath = (collectionId: string, limit = 100, offset = 0) => `/collections/${encodeURIComponent(collectionId)}/items?${new URLSearchParams({ limit: String(limit), offset: String(offset) }).toString()}`

export const apiBoxRecentItems = async (user_id: string) => {
  const data = await boxApiRequest<any>(user_id, buildBoxRecentItemsPath(), { method: 'GET' }, '获取 Box 最近文件失败')
  return Array.isArray(data?.entries) ? data.entries.map((item: any) => mapBoxItemToAliModel(item, 'box', item.parent?.id || 'box_root')) : []
}

export const apiBoxFavoriteItems = async (user_id: string) => {
  const collections = await boxApiRequest<{ entries?: BoxCollection[] }>(user_id, buildBoxCollectionsPath(), { method: 'GET' }, '获取 Box 收藏夹失败')
  const favorites = collections?.entries?.find((item) => item.type === 'favorites')
  if (!favorites?.id) return []
  const data = await boxApiRequest<any>(user_id, buildBoxCollectionItemsPath(favorites.id), { method: 'GET' }, '获取 Box 收藏文件失败')
  return Array.isArray(data?.entries) ? data.entries.map((item: any) => mapBoxItemToAliModel(item, 'box', item.parent?.id || 'box_root')) : []
}
