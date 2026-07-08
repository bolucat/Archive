export type BookFullTranslationMode = 'no' | 'both' | 'target'

export interface BookFullTranslationReader {
  getBatchTransTexts: () => Promise<string[]>
  handleBatchTransResult: (texts: string[], translations: string[]) => void
}

export interface BookFullTranslationControllerDeps {
  translate: (text: string, target: string, provider: string) => Promise<string>
  checkUsage: (characters: number) => { allowed: boolean; message?: string }
  onStateChange: (state: { loading: boolean; error?: string }) => void
}

export interface BookFullTranslationScheduleInput {
  reader: BookFullTranslationReader
  mode: BookFullTranslationMode
  provider: string
  target: string
}

export interface BookFullTranslationController {
  schedule: (input: BookFullTranslationScheduleInput) => Promise<void>
  invalidate: () => void
  clear: () => void
}

type TranslationState = {
  loading: boolean
  error?: string
}

function normalizeErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) {
    return error.message
  }
  if (typeof error === 'string' && error.trim()) {
    return error
  }
  return 'translation failed'
}

export function createBookFullTranslationController(deps: BookFullTranslationControllerDeps): BookFullTranslationController {
  const cache = new Map<string, string>()
  let generation = 0
  let queue = Promise.resolve()

  function emit(state: TranslationState) {
    try {
      deps.onStateChange(state)
    } catch {
      // State observers must not break scheduling or backfill.
    }
  }

  function invalidate() {
    generation += 1
    queue = Promise.resolve()
    emit({ loading: false })
  }

  function clear() {
    cache.clear()
    invalidate()
  }

  function cacheKey(provider: string, target: string, source: string) {
    return `${provider}\u0000${target}\u0000${source}`
  }

  async function runScheduledTask(input: BookFullTranslationScheduleInput, taskGeneration: number): Promise<void> {
    if (taskGeneration !== generation) {
      return
    }
    if (input.mode === 'no') {
      return
    }

    emit({ loading: true })

    try {
      const texts = await input.reader.getBatchTransTexts()
      if (taskGeneration !== generation) {
        return
      }
      if (texts.length === 0) {
        emit({ loading: false })
        return
      }

      const uniqueTexts: string[] = []
      const seen = new Set<string>()
      for (const text of texts) {
        if (seen.has(text)) {
          continue
        }
        seen.add(text)
        uniqueTexts.push(text)
      }

      const resolved = new Map<string, string>()
      const pending: string[] = []
      let characters = 0

      for (const text of uniqueTexts) {
        const cached = cache.get(cacheKey(input.provider, input.target, text))
        if (cached !== undefined) {
          resolved.set(text, cached)
          continue
        }
        pending.push(text)
        characters += text.length
      }

      const usage = deps.checkUsage(characters)
      if (!usage.allowed) {
        emit({ loading: false, error: usage.message || 'translation rejected' })
        return
      }

      const applyTranslation = (text: string, translated: string): boolean => {
        if (taskGeneration !== generation) {
          return false
        }
        input.reader.handleBatchTransResult([text], [translated])
        if (taskGeneration !== generation) {
          return false
        }
        cache.set(cacheKey(input.provider, input.target, text), translated)
        return true
      }

      for (const [text, translated] of resolved) {
        if (!applyTranslation(text, translated)) return
      }

      let firstError: unknown
      await Promise.all(
        pending.map(async (text) => {
          try {
            const translated = await deps.translate(text, input.target, input.provider)
            if (taskGeneration !== generation) return
            resolved.set(text, translated)
            applyTranslation(text, translated)
          } catch (error) {
            if (firstError === undefined) {
              firstError = error
            }
          }
        })
      )

      if (taskGeneration !== generation) {
        return
      }
      if (firstError !== undefined) {
        emit({ loading: false, error: normalizeErrorMessage(firstError) })
        return
      }
      emit({ loading: false })
    } catch (error) {
      if (taskGeneration !== generation) {
        return
      }
      emit({ loading: false, error: normalizeErrorMessage(error) })
    }
  }

  function schedule(input: BookFullTranslationScheduleInput): Promise<void> {
    if (input.mode === 'no') {
      return Promise.resolve()
    }

    const taskGeneration = generation
    const task = queue.then(() => runScheduledTask(input, taskGeneration))
    queue = task.catch(() => {})
    return task
  }

  return {
    schedule,
    invalidate,
    clear,
  }
}
