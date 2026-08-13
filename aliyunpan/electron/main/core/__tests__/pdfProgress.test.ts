import { describe, expect, it, vi } from 'vitest'
import { sendPdfProgress } from '../../documentInsight/pdfProgress'

describe('PDF extraction progress IPC', () => {
  it('does not send progress after the reader window is destroyed', () => {
    const sender = { isDestroyed: () => true, send: vi.fn() }
    sendPdfProgress(sender, 'request-1', { phase: 'download', current: 1 })
    expect(sender.send).not.toHaveBeenCalled()
  })

  it('ignores the close race when Electron rejects a progress send', () => {
    const sender = { isDestroyed: vi.fn(() => false), send: vi.fn(() => { throw new TypeError('Object has been destroyed') }) }
    expect(() => sendPdfProgress(sender, 'request-1', { phase: 'download', current: 1 })).not.toThrow()
  })

  it('preserves unrelated IPC errors', () => {
    const sender = { isDestroyed: () => false, send: vi.fn(() => { throw new Error('IPC unavailable') }) }
    expect(() => sendPdfProgress(sender, 'request-1', {})).toThrow('IPC unavailable')
  })
})
