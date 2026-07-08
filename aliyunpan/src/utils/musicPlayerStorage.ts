import type { IPageMusicTrack } from '../store/appstore'

export function musicTrackKey(t: Pick<IPageMusicTrack, 'user_id' | 'drive_id' | 'file_id'> | undefined | null): string {
  return t ? `${t.user_id || ''}|${t.drive_id || ''}|${t.file_id || ''}` : ''
}

function readJsonArray(key: string): IPageMusicTrack[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(key) || '[]')
    return Array.isArray(parsed) ? parsed.filter((t) => t?.file_id) : []
  } catch {
    return []
  }
}

export function loadMusicTrackList(key: string, opts: { legacyKeys?: string[] } = {}): IPageMusicTrack[] {
  const out: IPageMusicTrack[] = []
  const seen = new Set<string>()
  const append = (track: IPageMusicTrack) => {
    const k = musicTrackKey(track)
    if (!k || seen.has(k)) return
    seen.add(k)
    out.push(track)
  }
  for (const track of readJsonArray(key)) append(track)
  for (const legacyKey of opts.legacyKeys || []) {
    for (const track of readJsonArray(legacyKey)) append(track)
  }
  return out
}

export function saveMusicTrackList(key: string, tracks: IPageMusicTrack[]): void {
  try {
    localStorage.setItem(key, JSON.stringify(tracks.filter((t) => t?.file_id)))
  } catch {}
}
