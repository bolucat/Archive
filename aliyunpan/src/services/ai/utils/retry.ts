export interface RetryOptions {
  maxRetries?: number
  baseDelayMs?: number
  maxDelayMs?: number
  onRetry?: (attempt: number, error: unknown) => void
}

export async function withRetry<T>(fn: () => Promise<T>, options: RetryOptions = {}): Promise<T> {
  const { maxRetries = 3, baseDelayMs = 1000, maxDelayMs = 15000, onRetry } = options
  let lastError: unknown
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      return await fn()
    } catch (error) {
      lastError = error
      if (error instanceof DOMException && error.name === 'AbortError') throw error
      if (attempt === maxRetries) break
      const delay = Math.min(baseDelayMs * Math.pow(2, attempt), maxDelayMs)
      onRetry?.(attempt + 1, error)
      await new Promise((resolve) => setTimeout(resolve, delay))
    }
  }
  throw lastError
}

export async function withTimeout<T>(promise: Promise<T>, ms: number, message = 'Operation timed out'): Promise<T> {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), ms)
  try {
    const result = await Promise.race([
      promise,
      new Promise<never>((_, reject) => {
        controller.signal.addEventListener('abort', () => reject(new Error(message)))
      }),
    ])
    return result
  } finally {
    clearTimeout(timer)
  }
}

export async function withRetryAndTimeout<T>(fn: () => Promise<T>, timeoutMs: number, retryOptions?: RetryOptions): Promise<T> {
  return withRetry(() => withTimeout(fn(), timeoutMs), retryOptions)
}

export const AI_TIMEOUTS = {
  EMBEDDING_SINGLE: 30_000,
  EMBEDDING_BATCH: 120_000,
  CHAT_STREAM: 60_000,
  HEALTH_CHECK: 5_000,
  OLLAMA_CONNECT: 5_000,
} as const

export const AI_RETRY_CONFIGS: Record<string, RetryOptions> = {
  EMBEDDING: { maxRetries: 3, baseDelayMs: 2000, maxDelayMs: 15000 },
  CHAT: { maxRetries: 2, baseDelayMs: 1000, maxDelayMs: 5000 },
  HEALTH_CHECK: { maxRetries: 1, baseDelayMs: 500, maxDelayMs: 1000 },
}
