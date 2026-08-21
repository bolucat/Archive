import { describe, expect, it, vi } from 'vitest'

vi.mock('../../utils/message', () => ({ default: { error: vi.fn() } }))
vi.mock('../auth', () => ({ getBaiduToken: vi.fn() }))

import { resolveBaiduTargetPath } from '../filecmd'

describe('resolveBaiduTargetPath', () => {
  it('resolves a Baidu fs_id to its directory path', () => {
    expect(resolveBaiduTargetPath('38112063721', '', '', [{ file_id: '38112063721', path: '/影视/电视剧' }])).toBe('/影视/电视剧')
  })

  it('does not treat an unresolved fs_id as a root directory name', () => {
    expect(resolveBaiduTargetPath('483108569491', '', '', [])).toBe('')
  })

  it('uses the cached directory description when the fs_id is not in the current list', () => {
    expect(resolveBaiduTargetPath('483108569491', '', 'baidu_fsid:483108569491;baidu_path:/电影/待整理', [])).toBe('/电影/待整理')
  })
})
