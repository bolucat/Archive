import { describe, expect, it, vi } from 'vitest'

vi.mock('../../utils/message', () => ({ default: { error: vi.fn() } }))
vi.mock('../../config', () => ({ default: {} }))
vi.mock('../auth', () => ({ getCloud123Token: vi.fn() }))

import { getCloud123MkdirFileId } from '../filecmd'
import { getProviderCapabilities } from '../../services/agent/providerCapabilities'

describe('123 云盘文件操作', () => {
  it('reads the dirID returned by the create-folder endpoint', () => {
    expect(getCloud123MkdirFileId({ code: 0, data: { dirID: 44823630 } })).toBe('44823630')
  })

  it('does not advertise unsupported permanent trash deletion', () => {
    expect(getProviderCapabilities('cloud123').operations['trash.delete']).toBe(false)
  })
})
