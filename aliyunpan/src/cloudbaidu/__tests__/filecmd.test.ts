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
})
