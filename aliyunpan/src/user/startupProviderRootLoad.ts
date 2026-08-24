export const shouldRetryInitialRootLoad = (provider: string) => String(provider || '').toLowerCase() !== 'aliyun'

const waitForRootRetry = () => new Promise<void>(resolve => setTimeout(resolve, 600))

export async function loadInitialProviderRoot(shouldRetry: boolean, load: () => Promise<void>, wait: () => Promise<void> = waitForRootRetry): Promise<void> {
  try {
    await load()
  } catch (error) {
    if (!shouldRetry) throw error
    await wait()
    await load()
  }
}
