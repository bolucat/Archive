import { describe, expect, it, vi, beforeEach } from 'vitest'
import { createBookFullTranslationController, type BookFullTranslationMode } from '../bookFullTranslation'

type Reader = {
  getBatchTransTexts: () => Promise<string[]>
  handleBatchTransResult: (texts: string[], translations: string[]) => void
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

function createReader(
  texts: string[] | (() => Promise<string[]>),
  overrides: Partial<Reader> = {}
): Reader {
  return {
    getBatchTransTexts: typeof texts === 'function' ? texts : vi.fn(async () => texts),
    handleBatchTransResult: vi.fn(),
    ...overrides,
  }
}

describe('bookFullTranslation', () => {
  let translate: ReturnType<typeof vi.fn>
  let checkUsage: ReturnType<typeof vi.fn>
  let onStateChange: ReturnType<typeof vi.fn>

  beforeEach(() => {
    translate = vi.fn(async (text: string, target: string, provider: string) => `${provider}:${target}:${text}`)
    checkUsage = vi.fn((characters: number) => ({ allowed: characters <= 10, message: 'quota exceeded' }))
    onStateChange = vi.fn()
  })

  it('serializes schedules so the second batch waits for the first to finish', async () => {
    const first = deferred<string>()
    const second = deferred<string>()
    translate
      .mockImplementationOnce(async () => first.promise)
      .mockImplementationOnce(async () => second.promise)

    const controller = createBookFullTranslationController({
      translate,
      checkUsage,
      onStateChange,
    })

    const reader1 = createReader(['first'])
    const reader2 = createReader(['second'])

    const task1 = controller.schedule({ reader: reader1, mode: 'both', provider: 'p1', target: 'zh' })
    const task2 = controller.schedule({ reader: reader2, mode: 'target', provider: 'p1', target: 'zh' })

    await vi.waitFor(() => expect(translate).toHaveBeenCalledTimes(1))
    expect(translate).toHaveBeenCalledTimes(1)
    expect(translate).toHaveBeenCalledWith('first', 'zh', 'p1')

    first.resolve('one')
    await task1

    await vi.waitFor(() => expect(translate).toHaveBeenCalledTimes(2))
    expect(translate).toHaveBeenCalledTimes(2)
    expect(translate).toHaveBeenLastCalledWith('second', 'zh', 'p1')

    second.resolve('two')
    await task2

    expect(reader1.handleBatchTransResult).toHaveBeenCalledWith(['first'], ['one'])
    expect(reader2.handleBatchTransResult).toHaveBeenCalledWith(['second'], ['two'])
  })

  it('deduplicates texts within a batch and reuses cached translations for later batches', async () => {
    const controller = createBookFullTranslationController({
      translate,
      checkUsage,
      onStateChange,
    })

    const reader1 = createReader(['alpha', 'alpha', 'beta'])
    const reader2 = createReader(['beta', 'alpha'])

    await controller.schedule({ reader: reader1, mode: 'both', provider: 'p1', target: 'zh' })

    expect(checkUsage).toHaveBeenNthCalledWith(1, 9)
    expect(translate).toHaveBeenCalledTimes(2)
    expect(reader1.handleBatchTransResult).toHaveBeenCalledWith(['alpha'], ['p1:zh:alpha'])
    expect(reader1.handleBatchTransResult).toHaveBeenCalledWith(['beta'], ['p1:zh:beta'])

    await controller.schedule({ reader: reader2, mode: 'both', provider: 'p1', target: 'zh' })

    expect(checkUsage).toHaveBeenNthCalledWith(2, 0)
    expect(translate).toHaveBeenCalledTimes(2)
    expect(reader2.handleBatchTransResult).toHaveBeenCalledWith(['beta'], ['p1:zh:beta'])
    expect(reader2.handleBatchTransResult).toHaveBeenCalledWith(['alpha'], ['p1:zh:alpha'])
  })

  it('renders each translation as soon as that request resolves', async () => {
    const slow = deferred<string>()
    const fast = deferred<string>()
    translate.mockImplementation((text: string) => {
      if (text === 'slow') return slow.promise
      if (text === 'fast') return fast.promise
      return Promise.resolve(text)
    })
    const controller = createBookFullTranslationController({
      translate,
      checkUsage,
      onStateChange,
    })
    const reader = createReader(['slow', 'fast'])

    const task = controller.schedule({ reader, mode: 'both', provider: 'p1', target: 'zh' })

    await vi.waitFor(() => expect(translate).toHaveBeenCalledTimes(2))
    fast.resolve('FAST')
    await vi.waitFor(() => expect(reader.handleBatchTransResult).toHaveBeenCalledWith(['fast'], ['FAST']))
    expect(reader.handleBatchTransResult).not.toHaveBeenCalledWith(['slow'], ['SLOW'])

    slow.resolve('SLOW')
    await task

    expect(reader.handleBatchTransResult).toHaveBeenCalledWith(['slow'], ['SLOW'])
  })

  it('skips translation when mode is no', async () => {
    const controller = createBookFullTranslationController({
      translate,
      checkUsage,
      onStateChange,
    })

    const reader = createReader(['skip me'])

    await controller.schedule({ reader, mode: 'no' as BookFullTranslationMode, provider: 'p1', target: 'zh' })

    expect(translate).not.toHaveBeenCalled()
    expect(checkUsage).not.toHaveBeenCalled()
    expect(reader.handleBatchTransResult).not.toHaveBeenCalled()
  })

  it('rejects batches that exceed the allowed usage and reports the message', async () => {
    checkUsage.mockReturnValue({ allowed: false, message: 'too many characters' })
    const controller = createBookFullTranslationController({
      translate,
      checkUsage,
      onStateChange,
    })

    const reader = createReader(['one', 'two'])

    await controller.schedule({ reader, mode: 'both', provider: 'p1', target: 'zh' })

    expect(checkUsage).toHaveBeenCalledWith(6)
    expect(translate).not.toHaveBeenCalled()
    expect(reader.handleBatchTransResult).not.toHaveBeenCalled()
    expect(onStateChange).toHaveBeenCalledWith({ loading: false, error: 'too many characters' })
  })

  it('reports one error for a failed batch and leaves loading false', async () => {
    translate.mockImplementationOnce(async () => 'ok').mockImplementationOnce(async () => {
      throw new Error('boom')
    })
    const controller = createBookFullTranslationController({
      translate,
      checkUsage,
      onStateChange,
    })

    const reader = createReader(['good', 'bad', 'good'])

    await controller.schedule({ reader, mode: 'both', provider: 'p1', target: 'zh' })

    expect(translate).toHaveBeenCalledTimes(2)
    expect(reader.handleBatchTransResult).toHaveBeenCalledWith(['good'], ['ok'])
    expect(onStateChange.mock.calls.filter(([state]) => state.error).length).toBe(1)
    expect(onStateChange).toHaveBeenLastCalledWith({ loading: false, error: 'boom' })
  })

  it('invalidates queued work after getBatchTransTexts is already pending', async () => {
    const batch = deferred<string[]>()
    const getBatchTransTexts = vi.fn(() => batch.promise)
    const states: boolean[] = []
    onStateChange.mockImplementation((state) => {
      states.push(state.loading)
    })
    const controller = createBookFullTranslationController({
      translate,
      checkUsage,
      onStateChange,
    })

    const reader = createReader(getBatchTransTexts)
    const task = controller.schedule({ reader, mode: 'both', provider: 'p1', target: 'zh' })

    await vi.waitFor(() => expect(getBatchTransTexts).toHaveBeenCalledTimes(1))
    controller.invalidate()
    batch.resolve(['late'])
    await task

    expect(translate).not.toHaveBeenCalled()
    expect(reader.handleBatchTransResult).not.toHaveBeenCalled()
    expect(states).toEqual([true, false])
  })

  it('lets a new schedule complete while old in-flight translation is still pending after invalidate', async () => {
    const translated = deferred<string>()
    translate.mockImplementationOnce(async () => translated.promise)
    const controller = createBookFullTranslationController({
      translate,
      checkUsage,
      onStateChange,
    })

    const oldStates: boolean[] = []
    onStateChange.mockImplementation((state) => {
      if (typeof state.loading === 'boolean') {
        oldStates.push(state.loading)
      }
    })

    const oldReader = createReader(['late'])
    const oldTask = controller.schedule({ reader: oldReader, mode: 'both', provider: 'p1', target: 'zh' })

    await vi.waitFor(() => expect(translate).toHaveBeenCalledTimes(1))
    expect(translate).toHaveBeenCalledWith('late', 'zh', 'p1')
    expect(oldStates).toContain(true)

    controller.invalidate()
    const newReader = createReader(['fresh'])
    const newTask = controller.schedule({ reader: newReader, mode: 'both', provider: 'p1', target: 'zh' })
    await vi.waitFor(() => expect(translate).toHaveBeenCalledTimes(2))
    await newTask

    expect(newReader.handleBatchTransResult).toHaveBeenCalledWith(['fresh'], ['p1:zh:fresh'])
    expect(oldStates).toEqual([true, false, true, false])
    const statesBeforeOldResolve = oldStates.length

    translated.resolve('translated')
    await oldTask

    expect(oldReader.handleBatchTransResult).not.toHaveBeenCalled()
    expect(oldStates.length).toBe(statesBeforeOldResolve)
  })

  it('does not cache a batch when handleBatchTransResult throws', async () => {
    const reader1 = createReader(['cache me'], {
      handleBatchTransResult: vi.fn(() => {
        throw new Error('render failed')
      }),
    })
    const reader2 = createReader(['cache me'])
    const controller = createBookFullTranslationController({
      translate,
      checkUsage,
      onStateChange,
    })

    await controller.schedule({ reader: reader1, mode: 'both', provider: 'p1', target: 'zh' })
    await controller.schedule({ reader: reader2, mode: 'both', provider: 'p1', target: 'zh' })

    expect(translate).toHaveBeenCalledTimes(2)
    expect(reader2.handleBatchTransResult).toHaveBeenCalledWith(['cache me'], ['p1:zh:cache me'])
  })

  it('does not rebuild cache if handler clears the controller after a successful return', async () => {
    let controller: ReturnType<typeof createBookFullTranslationController>
    const reader1 = createReader(['same text'], {
      handleBatchTransResult: vi.fn(() => {
        controller.clear()
      }),
    })
    const reader2 = createReader(['same text'])
    controller = createBookFullTranslationController({
      translate,
      checkUsage,
      onStateChange,
    })

    await controller.schedule({ reader: reader1, mode: 'both', provider: 'p1', target: 'zh' })
    await controller.schedule({ reader: reader2, mode: 'both', provider: 'p1', target: 'zh' })

    expect(translate).toHaveBeenCalledTimes(2)
    expect(reader2.handleBatchTransResult).toHaveBeenCalledWith(['same text'], ['p1:zh:same text'])
  })

  it('keeps schedule working when onStateChange throws', async () => {
    onStateChange.mockImplementation(() => {
      throw new Error('observer failed')
    })
    const controller = createBookFullTranslationController({
      translate,
      checkUsage,
      onStateChange,
    })

    const reader = createReader(['steady'])
    await expect(controller.schedule({ reader, mode: 'both', provider: 'p1', target: 'zh' })).resolves.toBeUndefined()

    expect(translate).toHaveBeenCalledTimes(1)
    expect(reader.handleBatchTransResult).toHaveBeenCalledWith(['steady'], ['p1:zh:steady'])
  })
})
