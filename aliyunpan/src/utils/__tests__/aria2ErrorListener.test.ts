import { describe, expect, it, vi } from 'vitest'

describe('Aria2 connection errors', () => {
  it('consumes an undefined EventEmitter error and marks the connection offline', async () => {
    vi.stubGlobal('self', globalThis)
    const { bindAriaErrorListener } = await import('../aria2c')
    let listener: ((error?: unknown) => void) | undefined
    const engine = {
      on: vi.fn((event: string, callback: (error?: unknown) => void) => {
        if (event === 'error') listener = callback
      })
    }
    const onDisconnect = vi.fn()

    bindAriaErrorListener(engine, 'local', onDisconnect)
    expect(() => listener?.(undefined)).not.toThrow()
    expect(onDisconnect).toHaveBeenCalledWith('local')
  })
})
