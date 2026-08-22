import { executeNextAgentCliGrant } from './AgentCliExecutor'
import { agentV1FeatureFlags } from './AgentV1FeatureFlags'
import { reconcileWorkspaceAgentV1Execution } from '../workspaceAgent/WorkspaceAgentV1Reconciler'

const DEFAULT_INTERVAL_MS = 3_000

export interface AgentCliExecutionScheduler {
  start(): void
  stop(): void
}

export function createAgentCliExecutionScheduler(options: { executeNext: () => Promise<unknown>; intervalMs?: number; onError?: (error: unknown) => void }): AgentCliExecutionScheduler {
  const intervalMs = options.intervalMs || DEFAULT_INTERVAL_MS
  let timer: ReturnType<typeof setInterval> | undefined
  let running = false
  let stopped = true

  const tick = async () => {
    if (stopped || running) return
    running = true
    try {
      await options.executeNext()
    } catch (error) {
      options.onError?.(error)
    } finally {
      running = false
    }
  }

  return {
    start() {
      if (timer) return
      stopped = false
      void tick()
      timer = setInterval(() => { void tick() }, intervalMs)
    },
    stop() {
      stopped = true
      if (timer) clearInterval(timer)
      timer = undefined
    }
  }
}

let scheduler: AgentCliExecutionScheduler | undefined

/** Starts only for the explicit V1 cutover flag; shadow bridges cannot execute. */
export function startAgentCliExecutionScheduler(): void {
  if (scheduler || !agentV1FeatureFlags().cliExecutor) return
  scheduler = createAgentCliExecutionScheduler({
    executeNext: async () => {
      const result = await executeNextAgentCliGrant()
      reconcileWorkspaceAgentV1Execution(result)
      return result
    },
    onError: error => console.error('[Agent V1] CLI executor failed', error)
  })
  scheduler.start()
}

export function stopAgentCliExecutionScheduler(): void {
  scheduler?.stop()
  scheduler = undefined
}
