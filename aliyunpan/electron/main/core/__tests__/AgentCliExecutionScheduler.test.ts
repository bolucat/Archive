import { afterEach, describe, expect, it, vi } from 'vitest'
import { createAgentCliExecutionScheduler } from '../../agent/AgentCliExecutionScheduler'

afterEach(() => vi.useRealTimers())

describe('AgentCliExecutionScheduler', () => {
  it('runs only one main-process grant execution at a time and stops cleanly', async () => {
    vi.useFakeTimers()
    let resolve!: () => void
    const executeNext = vi.fn(() => new Promise<void>(done => { resolve = done }))
    const scheduler = createAgentCliExecutionScheduler({ executeNext, intervalMs: 100 })

    scheduler.start()
    expect(executeNext).toHaveBeenCalledTimes(1)
    await vi.advanceTimersByTimeAsync(300)
    expect(executeNext).toHaveBeenCalledTimes(1)

    resolve()
    await Promise.resolve()
    await vi.advanceTimersByTimeAsync(100)
    expect(executeNext).toHaveBeenCalledTimes(2)

    scheduler.stop()
    resolve()
    await vi.advanceTimersByTimeAsync(500)
    expect(executeNext).toHaveBeenCalledTimes(2)
  })
})
