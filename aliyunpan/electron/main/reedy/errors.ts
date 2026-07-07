export class ReedyError extends Error {
  constructor(
    message: string,
    public readonly code: ReedyErrorCode,
    public readonly turnId?: string
  ) {
    super(message)
    this.name = 'ReedyError'
  }
}

export type ReedyErrorCode =
  | 'context_overflow'
  | 'turn_timeout'
  | 'model_error'
  | 'tool_error'
  | 'indexing_failed'
  | 'not_indexed'
  | 'empty_index'
  | 'stale_index'
  | 'embedding_failed'
  | 'embedding_timeout'
  | 'fts_failed'
  | 'db_error'
  | 'aborted'
  | 'budget_exceeded'
  | 'invalid_args'

export class ReedyToolError extends Error {
  constructor(
    message: string,
    public readonly code: ReedyToolErrorCode,
    public readonly toolName: string
  ) {
    super(message)
    this.name = 'ReedyToolError'
  }
}

export type ReedyToolErrorCode =
  | 'invalid_args'
  | 'permission_denied'
  | 'timeout'
  | 'aborted'
  | 'runtime'
  | 'not_available'
