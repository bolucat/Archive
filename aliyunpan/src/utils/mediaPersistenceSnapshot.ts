import { toRaw } from 'vue'

export class MediaPersistenceError extends Error {
  constructor(cause: unknown) {
    super(cause instanceof Error ? cause.message : String(cause), { cause })
    this.name = 'MediaPersistenceError'
  }
}

// IndexedDB cannot clone proxies nested in shallow copies. Preserve Dates/undefined.
export function mediaPersistenceSnapshot<T>(value: T): T {
  const seen = new WeakMap<object, any>()
  const snapshot = (input: any): any => {
    if (input === null || typeof input !== 'object') return input
    const raw = toRaw(input)
    if (raw instanceof Date) return new Date(raw.getTime())
    if (seen.has(raw)) return seen.get(raw)
    const result: any = Array.isArray(raw) ? [] : {}
    seen.set(raw, result)
    for (const key of Object.keys(raw)) result[key] = snapshot(raw[key])
    return result
  }
  return snapshot(value)
}
