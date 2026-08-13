import { describe, expect, it } from 'vitest'
import { associateMediaSubtitles } from '../mediaSubtitleAssociation'
import type { DriveFileItem } from '../../types/media'

const file = (id: string, parentFileId: string): DriveFileItem => ({ id, name: id, path: id, parentFileId, driveId: 'drive', driveServerId: 'server', fileSize: 1 })

describe('associateMediaSubtitles', () => {
  it('associates only same-directory subtitles in same-folder mode', () => {
    const video = file('video', 'season')
    const direct = file('direct.ass', 'season')
    const nested = file('nested.ass', 'subs')

    expect(associateMediaSubtitles(video, [direct, nested], new Map([['subs', 'season']]), 'same-folder').subtitleFiles).toEqual([direct])
  })

  it('also associates immediate child-folder subtitles when configured', () => {
    const video = file('video', 'season')
    const direct = file('direct.ass', 'season')
    const nested = file('nested.ass', 'subs')

    expect(associateMediaSubtitles(video, [direct, nested], new Map([['subs', 'season']]), 'include-subfolders').subtitleFiles).toEqual([direct, nested])
  })
})
