import { describe, expect, it } from 'vitest'
import {
  callAriaClient,
  getAriaAddUriGid,
  isAriaDuplicateGidError,
  shouldRemoveAriaStoppedResult
} from '../aria2Rpc'

describe('aria2 rpc helpers', () => {
  it('extracts gid from aria2.addUri multicall result shapes', () => {
    expect(getAriaAddUriGid('0123456789abcdef')).toBe('0123456789abcdef')
    expect(getAriaAddUriGid(['0123456789abcdef'])).toBe('0123456789abcdef')
    expect(getAriaAddUriGid([['0123456789abcdef']])).toBe('0123456789abcdef')
  })

  it('does not treat empty or error addUri results as gid success', () => {
    expect(getAriaAddUriGid(undefined)).toBe('')
    expect(getAriaAddUriGid({ code: 1, message: 'failed' })).toBe('')
    expect(getAriaAddUriGid([''])).toBe('')
  })

  it('detects duplicate gid errors without requiring an exact message', () => {
    expect(isAriaDuplicateGidError({ message: 'GID 0123456789abcdef already exists' })).toBe(true)
    expect(isAriaDuplicateGidError({ message: 'gid already exists' })).toBe(true)
    expect(isAriaDuplicateGidError({ message: 'network timeout' })).toBe(false)
  })

  it('keeps paused tasks in remote aria2 instead of removing their result', () => {
    expect(shouldRemoveAriaStoppedResult('paused')).toBe(false)
    expect(shouldRemoveAriaStoppedResult('error')).toBe(true)
    expect(shouldRemoveAriaStoppedResult('removed')).toBe(true)
  })

  it('does not throw when aria2 client is unavailable', async () => {
    await expect(callAriaClient(undefined, 'aria2.addTorrent', 'base64', [], {})).resolves.toBeUndefined()
  })

  it('captures aria2 client call errors for callers that need fallback logic', async () => {
    const error = new Error('connection closed')
    const client = {
      call: async () => {
        throw error
      }
    }
    let captured: unknown

    const result = await callAriaClient(client, 'aria2.addTorrent', 'base64', [], {}, (err: unknown) => {
      captured = err
    })

    expect(result).toBeUndefined()
    expect(captured).toBe(error)
  })
})
