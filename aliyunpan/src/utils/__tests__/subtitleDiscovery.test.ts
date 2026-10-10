import { describe, expect, it, vi } from 'vitest'
import { discoverSubtitleFiles, subtitleFileKey } from '../subtitleDiscovery'

const directory = (id: string, name = id) => ({ file_id: id, name, isDir: true })
const subtitle = (id: string, name = `${id}.srt`) => ({ file_id: id, name, drive_id: 'drive', user_id: 'account' })

describe('subtitle directory discovery', () => {
  it('keeps same-folder scope out of subfolders and recognizes extensionless provider metadata', async () => {
    const list = vi.fn(async () => [directory('subs'), subtitle('one', 'one.SRT'), subtitle('video', 'one.mkv')])
    expect((await discoverSubtitleFiles('root', false, list)).map(file => file.file_id)).toEqual(['one'])
    expect(list).toHaveBeenCalledTimes(1)
  })

  it('lists nested subtitles with relative paths and retains same names in different folders', async () => {
    const tree: Record<string, any[]> = {
      root: [directory('subs', '字幕'), subtitle('sibling', '03.srt')],
      subs: [subtitle('child', '03.srt'), directory('zh', '中文')],
      zh: [subtitle('nested', '03.ASS'), directory('subs')]
    }
    const list = vi.fn(async (id: string) => tree[id])
    const files = await discoverSubtitleFiles('root', true, list)
    expect(files.map(file => file.html)).toEqual(['03.srt', '字幕/03.srt', '字幕/中文/03.ASS'])
    expect(list).toHaveBeenCalledTimes(3)
  })

  it('discards an old scan after playback switches', async () => {
    let current = true
    const list = vi.fn(async () => { current = false; return [subtitle('old'), directory('nested')] })
    expect(await discoverSubtitleFiles('root', true, list, () => current)).toEqual([])
    expect(list).toHaveBeenCalledTimes(1)
  })

  it('does not treat directories or partial extensions as subtitles', async () => {
    const list = async () => [directory('folder', 'folder.srt'), subtitle('bad', 'file.srt.bak'), subtitle('good', 'file.vtt')]
    expect((await discoverSubtitleFiles('root', false, list)).map(file => file.file_id)).toEqual(['good'])
  })

  it('keeps account and drive identities separate', () => {
    const file = subtitle('shared-id')
    expect(subtitleFileKey(file)).not.toBe(subtitleFileKey({ ...file, user_id: 'another' }))
    expect(subtitleFileKey(file)).not.toBe(subtitleFileKey({ ...file, drive_id: 'another' }))
  })
})
