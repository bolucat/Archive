import { describe, expect, it } from 'vitest'
import { startBackgroundStartupTasks, withStartupTimeout } from '../startupTask'

describe('startup task timeout', () => {
  it('does not let one stalled provider block application startup', async () => {
    await expect(withStartupTimeout(new Promise<never>(() => {}), 'provider startup', 1)).rejects.toThrow('provider startup timed out')
  })

  it('starts later post-login work even when an earlier task never resolves', async () => {
    const completed: string[] = []
    startBackgroundStartupTasks([
      { label: 'stalled', run: () => new Promise<void>(() => {}) },
      { label: 'ready', run: async () => { completed.push('ready') } }
    ], () => {})

    await Promise.resolve()
    expect(completed).toEqual(['ready'])
  })
})
