export const getAriaAddUriGid = (result: unknown): string => {
  if (Array.isArray(result)) {
    for (const item of result) {
      const gid = getAriaAddUriGid(item)
      if (gid) return gid
    }
    return ''
  }
  if (typeof result === 'string') return result.trim()
  return ''
}

export const callAriaClient = async (
  client: { call: (method: string, ...args: any[]) => Promise<unknown> } | undefined,
  method: string,
  ...argsAndMaybeOnError: any[]
): Promise<unknown> => {
  const maybeOnError = argsAndMaybeOnError[argsAndMaybeOnError.length - 1]
  const onError = typeof maybeOnError === 'function' ? maybeOnError as (error: unknown) => void : undefined
  const args = onError ? argsAndMaybeOnError.slice(0, -1) : argsAndMaybeOnError
  if (!client) return undefined
  try {
    return await client.call(method, ...args)
  } catch (error) {
    onError?.(error)
    return undefined
  }
}

export const isAriaDuplicateGidError = (error: unknown): boolean => {
  const message = typeof error === 'object' && error && 'message' in error
    ? String((error as { message?: unknown }).message || '')
    : typeof error === 'string'
      ? error
      : ''
  return /\bgid\b/i.test(message) && /already exists/i.test(message)
}

export const shouldRemoveAriaStoppedResult = (status: string): boolean => {
  return status === 'error' || status === 'removed'
}
