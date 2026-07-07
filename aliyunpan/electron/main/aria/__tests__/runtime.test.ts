import { afterEach, describe, expect, it } from 'vitest'
import { ENGINE_RPC_PORT } from '@shared/constants'
import { getMotrixApplicationRpcPort } from '../runtime'

describe('Motrix aria runtime', () => {
  afterEach(() => {
    delete (globalThis as any).motrixApplication
  })

  it('returns MotrixApplication rpc-listen-port when initialized', () => {
    ;(globalThis as any).motrixApplication = {
      configManager: {
        getSystemConfig: (key: string) => key === 'rpc-listen-port' ? 16999 : undefined
      }
    }

    expect(getMotrixApplicationRpcPort()).toBe(16999)
  })

  it('falls back to the Motrix default rpc port before initialization', () => {
    expect(getMotrixApplicationRpcPort()).toBe(ENGINE_RPC_PORT)
  })
})
