/**
 * Browser-only subset of `@earendil-works/pi-ai/compat` required by
 * pi-agent-core's Agent and agent-loop modules. The upstream compat entry
 * eagerly registers server-oriented providers, some of which probe node:fs
 * and node:os during application startup. Renderer requests always provide
 * their own streamFn and go through the BoxPlayer AI Worker.
 */
export class EventStream<T, TResult = unknown> implements AsyncIterable<T> {
  private queue: T[] = []
  private waiting: Array<(result: IteratorResult<T>) => void> = []
  private done = false
  private readonly finalResultPromise: Promise<TResult>
  private resolveFinalResult!: (result: TResult) => void

  constructor(private readonly isComplete: (event: T) => boolean, private readonly extractResult: (event: T) => TResult) {
    this.finalResultPromise = new Promise(resolve => { this.resolveFinalResult = resolve })
  }

  push(event: T): void {
    if (this.done) return
    if (this.isComplete(event)) {
      this.done = true
      this.resolveFinalResult(this.extractResult(event))
    }
    const waiter = this.waiting.shift()
    if (waiter) waiter({ value: event, done: false })
    else this.queue.push(event)
  }

  end(result?: TResult): void {
    this.done = true
    if (result !== undefined) this.resolveFinalResult(result)
    while (this.waiting.length) this.waiting.shift()?.({ value: undefined as never, done: true })
  }

  async *[Symbol.asyncIterator](): AsyncIterator<T> {
    while (true) {
      if (this.queue.length) yield this.queue.shift()!
      else if (this.done) return
      else {
        const next = await new Promise<IteratorResult<T>>(resolve => this.waiting.push(resolve))
        if (next.done) return
        yield next.value
      }
    }
  }

  result(): Promise<TResult> {
    return this.finalResultPromise
  }
}

export function validateToolArguments(tool: { name: string; parameters: object }, toolCall: { arguments: unknown }): unknown {
  const args = structuredClone(toolCall.arguments)
  if (!matchesJsonSchema(tool.parameters as JsonSchema, args)) {
    throw new Error(`Validation failed for tool "${tool.name}"`)
  }
  return args
}

type JsonSchema = {
  type?: string
  required?: string[]
  properties?: Record<string, JsonSchema>
  items?: JsonSchema
  enum?: unknown[]
  anyOf?: JsonSchema[]
  oneOf?: JsonSchema[]
  allOf?: JsonSchema[]
}

function matchesJsonSchema(schema: JsonSchema, value: unknown): boolean {
  if (schema.anyOf?.length) return schema.anyOf.some(item => matchesJsonSchema(item, value))
  if (schema.oneOf?.length) return schema.oneOf.some(item => matchesJsonSchema(item, value))
  if (schema.allOf?.length && !schema.allOf.every(item => matchesJsonSchema(item, value))) return false
  if (schema.enum && !schema.enum.some(item => Object.is(item, value))) return false
  if (schema.type === 'string' && typeof value !== 'string') return false
  if (schema.type === 'number' && (typeof value !== 'number' || !Number.isFinite(value))) return false
  if (schema.type === 'integer' && (typeof value !== 'number' || !Number.isInteger(value))) return false
  if (schema.type === 'boolean' && typeof value !== 'boolean') return false
  if (schema.type === 'array') return Array.isArray(value) && (!schema.items || value.every(item => matchesJsonSchema(schema.items!, item)))
  if (schema.type === 'object') {
    if (!value || typeof value !== 'object' || Array.isArray(value)) return false
    const record = value as Record<string, unknown>
    if (schema.required?.some(key => !(key in record))) return false
    return !schema.properties || Object.entries(schema.properties).every(([key, property]) => !(key in record) || matchesJsonSchema(property, record[key]))
  }
  return true
}

export function streamSimple(): never {
  throw new Error('BoxPlayer Agent requires its Worker-backed streamFn in the renderer.')
}
