export const registerDrive115PlaybackAuth = (urls: string[], headers?: Record<string, string>, expiresAt?: number) => {
  if (typeof window === 'undefined') return
  const authorization = headers?.Authorization || headers?.authorization || ''
  const userAgent = headers?.['User-Agent'] || headers?.['user-agent'] || ''
  if (!urls.length || !authorization || !userAgent) return
  window.Electron?.ipcRenderer?.send('Drive115:RegisterPlaybackAuth', {
    urls,
    authorization,
    userAgent,
    expiresAt
  })
}
