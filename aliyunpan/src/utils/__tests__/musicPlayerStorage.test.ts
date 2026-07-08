import { describe, expect, it, vi } from 'vitest'
import { loadMusicTrackList } from '../musicPlayerStorage'

function stubLocalStorage(values: Record<string, string>) {
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => values[key] ?? null,
    setItem: vi.fn(),
    removeItem: vi.fn(),
    clear: vi.fn(),
    key: vi.fn(),
    length: Object.keys(values).length
  })
}

describe('musicPlayerStorage', () => {
  it('loads current and legacy favorite storage by the netdisk track key', () => {
    stubLocalStorage({
      'pm.favs': JSON.stringify([
        { user_id: 'u1', drive_id: 'd1', file_id: 'f1', file_name: 'A.mp3' }
      ]),
      'pageMusic.favorites': JSON.stringify([
        { user_id: 'u1', drive_id: 'd1', file_id: 'f1', file_name: 'A duplicate.mp3' },
        { user_id: 'u2', drive_id: 'd2', file_id: 'f2', file_name: 'B.mp3' }
      ])
    })

    const favs = loadMusicTrackList('pm.favs', { legacyKeys: ['pageMusic.favorites'] })

    expect(favs.map((track) => `${track.user_id}|${track.drive_id}|${track.file_id}`)).toEqual([
      'u1|d1|f1',
      'u2|d2|f2'
    ])
  })
})
