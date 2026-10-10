export interface SeriesMedia { id: string; title: string; serverId?: string; parentId?: string }
export interface CustomMediaSeries { id: string; title: string; members: SeriesMedia[] }
export const CUSTOM_SERIES_KEY = 'MediaLibrary_CustomSeries_v1'
export function seriesMediaKey(item: SeriesMedia): string { return JSON.stringify([item.serverId || 'local', item.id]) }
export function saveCustomSeries(groups: CustomMediaSeries[]): void { localStorage.setItem(CUSTOM_SERIES_KEY, JSON.stringify(groups)); window.dispatchEvent(new Event('boxplayer:custom-series-changed')) }
export function loadCustomSeries(): CustomMediaSeries[] { try { const data = JSON.parse(localStorage.getItem(CUSTOM_SERIES_KEY) || '[]'); return Array.isArray(data) ? data.filter(g => typeof g.id === 'string' && typeof g.title === 'string' && Array.isArray(g.members)) : [] } catch { return [] } }
export function openCustomSeries(item: SeriesMedia): void { window.dispatchEvent(new CustomEvent('boxplayer:custom-series', { detail: item })) }
export function toggleSeriesMember(group: CustomMediaSeries, item: SeriesMedia): CustomMediaSeries { const key = seriesMediaKey(item); const exists = group.members.some(member => seriesMediaKey(member) === key); return { ...group, members: exists ? group.members.filter(member => seriesMediaKey(member) !== key) : [...group.members, { ...item }] } }
