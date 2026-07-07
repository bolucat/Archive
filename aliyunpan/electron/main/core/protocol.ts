import { app } from 'electron'
import type { BrowserWindow } from 'electron'

export const extractExternalDownloadArg = (argv: string[]): string =>
  argv.find((arg) =>
    /^magnet:\?/i.test(arg) ||
    /\.torrent$/i.test(arg) ||
    /^https?:\/\/.+\.torrent(?:[?#].*)?$/i.test(arg)
  ) || ''

export const sendExternalDownloadToWindow = (win: BrowserWindow | null | undefined, payload: string): void => {
  if (!win || !payload) return
  try { win.webContents.send('external-download:open', payload) } catch {}
}

export const registerExternalDownloadProtocol = (getMainWindow: () => BrowserWindow | null | undefined): void => {
  app.on('open-url', (event, url) => {
    event.preventDefault()
    sendExternalDownloadToWindow(getMainWindow(), url)
  })

  app.on('open-file', (event, filePath) => {
    if (!/\.torrent$/i.test(filePath)) return
    event.preventDefault()
    sendExternalDownloadToWindow(getMainWindow(), filePath)
  })
}
