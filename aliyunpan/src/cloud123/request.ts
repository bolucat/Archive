type AccessToken = { access_token: string }

/** Local expiry metadata can be stale after restoring an account. Retry only
 * an explicit authentication rejection, once, with a freshly resolved token. */
export async function fetchCloud123JsonWithAuthRetry(url: string, token: AccessToken, refresh: () => Promise<AccessToken | null>) {
  let current = token
  for (let attempt = 0; ; attempt++) {
    const response = await fetch(url, {
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${current.access_token}`, Platform: 'open_platform' }
    })
    const data = await response.json().catch(() => undefined)
    const tokenRejected = response.status === 401 || /access[ _-]*token.*(?:invalid|expired)|(?:invalid|expired).*access[ _-]*token/i.test(String(data?.message || ''))
    if (!attempt && tokenRejected) {
      const refreshed = await refresh()
      if (refreshed?.access_token) { current = refreshed; continue }
    }
    return { response, data }
  }
}
