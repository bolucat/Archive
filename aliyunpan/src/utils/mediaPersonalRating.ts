export const PERSONAL_RATING_KEY = 'MediaLibrary_PersonalRatings_v1'
export function readPersonalRatings(): Record<string, number> {
  const value = JSON.parse(localStorage.getItem(PERSONAL_RATING_KEY) || '{}')
  if (!value || Array.isArray(value) || typeof value !== 'object') throw new Error('Invalid personal ratings')
  return Object.fromEntries(Object.entries(value).filter(([, rating]) => typeof rating === 'number' && Number.isInteger(rating) && rating >= 1 && rating <= 10)) as Record<string, number>
}
export function savePersonalRating(id: string, rating: number): void {
  if (!id || !Number.isInteger(rating) || rating < 1 || rating > 10) throw new Error('Invalid personal rating')
  localStorage.setItem(PERSONAL_RATING_KEY, JSON.stringify({ ...readPersonalRatings(), [id]: rating }))
}
export function openPersonalRating(item: { id: string; name: string }): void { window.dispatchEvent(new CustomEvent('boxplayer:personal-rating', { detail: item })) }
export function openServerPersonalRating(item: import('../types/mediaServerContent').MediaServerCardItem): void {
  openPersonalRating({ ...item, id: `${item.serverId}:${item.id}`, name: item.title, type: item.kind === 'movie' ? 'movie' : item.kind === 'series' ? 'tv' : item.kind === 'episode' ? 'episode' : undefined } as { id: string; name: string })
}
