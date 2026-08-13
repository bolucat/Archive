import { app } from 'electron'
import type { BrowserWindow } from 'electron'
import { pathToFileURL } from 'node:url'

const EXTERNAL_MEDIA_FILE_RE = /\.(?:mp4|mkv|avi|mov|webm|ts|m2ts|flv|wmv|mpg|mpeg|mp3|flac|m4a|aac|wav|ogg|opus|wma|aiff|ape|epub|pdf|mobi|azw|azw3|fb2|txt|md|markdown|docx|html|htm|cbz|cbr|cbt|cb7)$/i

export interface ExternalFilePayload {
  filePath: string
  fileUrl: string
}

let externalFileWindowGetter: (() => BrowserWindow | null | undefined) | undefined
let externalFileRegistered = false
let externalFileFlushPending = false
const pendingExternalFiles: ExternalFilePayload[] = []

export const extractExternalFileArg = (argv: string[]): string => argv.find((arg) => !arg.startsWith('-') && EXTERNAL_MEDIA_FILE_RE.test(arg)) || ''

function flushExternalFiles() {
  if (externalFileFlushPending) return
  const win = externalFileWindowGetter?.()
  if (!win || win.isDestroyed()) return
  if (win.webContents.isLoadingMainFrame()) {
    externalFileFlushPending = true
    win.webContents.once('did-finish-load', () => {
      externalFileFlushPending = false
      flushExternalFiles()
    })
    return
  }
  while (pendingExternalFiles.length) {
    const payload = pendingExternalFiles.shift()
    if (payload) win.webContents.send('external-file:open', payload)
  }
}

export function openExternalFile(filePath: string) {
  if (!filePath || !EXTERNAL_MEDIA_FILE_RE.test(filePath)) return
  pendingExternalFiles.push({ filePath, fileUrl: pathToFileURL(filePath).href })
  flushExternalFiles()
}

export const registerExternalFileProtocol = (getMainWindow: () => BrowserWindow | null | undefined): void => {
  externalFileWindowGetter = getMainWindow
  if (!externalFileRegistered) {
    externalFileRegistered = true
    app.on('open-file', (event, filePath) => {
      if (!EXTERNAL_MEDIA_FILE_RE.test(filePath)) return
      event.preventDefault()
      openExternalFile(filePath)
    })
    const initialFile = extractExternalFileArg(process.argv)
    if (initialFile) openExternalFile(initialFile)
  }
  flushExternalFiles()
}

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
