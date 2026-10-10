import { describe, expect, it } from 'vitest'
import { addTracksToList, createPlaylist } from '../radio/LocalPlaylistManager'
import type { IPageMusicTrack } from '../../store/appstore'

const track = (user_id: string): IPageMusicTrack => ({ user_id, drive_id: 'root', file_id: 'same-id', parent_file_id: '', file_name: 'song.flac', password: '', encType: '' })
describe('music playlist membership', () => {
  it('keeps equal file IDs from different accounts and deduplicates incoming tracks', () => {
    const original = createPlaylist('音乐', [track('a')])
    const updated = addTracksToList(original, [track('a'), track('b'), track('b')])
    expect(updated.tracks.map(t => t.user_id)).toEqual(['a', 'b'])
    expect(original.tracks).toHaveLength(1)
  })
  it('creates an empty playlist without silently adding library songs', () => {
    expect(createPlaylist('新歌单').tracks).toEqual([])
  })
})
