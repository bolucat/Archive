const inFlightRefreshes = new Map<string, Promise<unknown>>()

export const tokenRefreshKey = (provider: string, identity: string): string => `${provider}:${identity}`

export const withTokenRefreshLock = async <T>(key: string, refresh: () => Promise<T>): Promise<T> => {
  const pending = inFlightRefreshes.get(key)
  if (pending) return pending as Promise<T>

  const current = refresh().finally(() => {
    if (inFlightRefreshes.get(key) === current) inFlightRefreshes.delete(key)
  })
  inFlightRefreshes.set(key, current)
  return current
}
