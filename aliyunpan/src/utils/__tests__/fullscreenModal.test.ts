import { describe, expect, it } from 'vitest'
import { resolveFullscreenModalContainer } from '../fullscreenModal'

describe('resolveFullscreenModalContainer', () => {
  it('uses the fullscreen root only while the player is in native fullscreen', () => {
    const root = { id: 'art-player' }

    expect(resolveFullscreenModalContainer(true, root)).toBe(root)
    expect(resolveFullscreenModalContainer(false, root)).toBeUndefined()
  })
})
