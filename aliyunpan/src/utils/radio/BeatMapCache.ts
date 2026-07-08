import type { IPageMusicTrack } from '../../store/appstore'
import { getMineradioValue, setMineradioValue } from './MineradioStorage'

export interface CachedBeatMap {
  mode: 'mr' | 'dj'
  bpm: number
  peaks: number
  peakTimes: number[]
  beats?: MineradioBeatEvent[]
  pulseBeats?: MineradioBeatEvent[]
  cameraBeats?: MineradioBeatEvent[]
  duration: number
  updatedAt: number
}

export interface MineradioBeatEvent {
  time: number
  strength: number
  confidence: number
  impact: number
  combo: 'downbeat' | 'push' | 'drop' | 'rebound' | 'accent'
  low: number
  body: number
  snap: number
  mass: number
  sharpness: number
  pulse: boolean
  camera: boolean
  primary: boolean
  index: number
}

export function beatMapCacheKey(track: IPageMusicTrack | undefined | null, mode: 'mr' | 'dj', duration = 0) {
  if (!track) return ''
  const dur = Number.isFinite(duration) && duration > 0 ? Math.round(duration) : 0
  return ['beatmap', mode, track.user_id || '', track.drive_id || '', track.file_id || '', track.file_name || '', dur].join('|')
}

export async function getCachedBeatMap(track: IPageMusicTrack | undefined | null, mode: 'mr' | 'dj', duration = 0) {
  const key = beatMapCacheKey(track, mode, duration)
  if (!key) return null
  return getMineradioValue<CachedBeatMap>(key).catch(() => null)
}

export async function setCachedBeatMap(track: IPageMusicTrack | undefined | null, mode: 'mr' | 'dj', duration: number, value: Omit<CachedBeatMap, 'mode' | 'duration' | 'updatedAt'>) {
  const key = beatMapCacheKey(track, mode, duration)
  if (!key) return
  await setMineradioValue<CachedBeatMap>(key, {
    ...value,
    mode,
    duration: Number.isFinite(duration) ? duration : 0,
    updatedAt: Date.now()
  })
}
