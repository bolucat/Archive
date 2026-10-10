import type { MediaServerCardItem } from '../types/mediaServerContent'

export const libraryCover = (item: MediaServerCardItem) =>
  item.images?.primary || item.poster || item.images?.thumb || item.images?.backdrop || item.backdrop || ''

export const withLibraryCoverFallback = <T extends MediaServerCardItem>(library: T, items: MediaServerCardItem[]): T => {
  if (libraryCover(library)) return library
  const candidates = items.filter((item) => item.images?.primary || item.poster)
  if (!candidates.length) return library
  const item = candidates[Math.floor(Math.random() * candidates.length)]
  const primary = item.images?.primary || item.poster
  return { ...library, poster: primary, images: { ...library.images, primary } }
}
