import { recordMetric } from './ReedyService'

let flushTimer: ReturnType<typeof setTimeout> | null = null
const pendingEvents: Array<{
  ts: number
  event: string
  book_hash?: string
  session_id?: string
  turn_id?: string
  message_id?: string
  payload?: string
}> = []

const FLUSH_INTERVAL_MS = 2000

function flush() {
  const batch = pendingEvents.splice(0)
  for (const evt of batch) {
    try {
      recordMetric(evt)
    } catch {
      // silent
    }
  }
}

export function track(event: string, data?: Record<string, unknown>) {
  pendingEvents.push({
    ts: Date.now(),
    event,
    ...data
  })

  if (flushTimer === null) {
    flushTimer = setTimeout(() => {
      flush()
      flushTimer = null
    }, FLUSH_INTERVAL_MS)
  }

  if (pendingEvents.length >= 20) {
    flush()
  }
}

export { getMetrics } from './ReedyService'
