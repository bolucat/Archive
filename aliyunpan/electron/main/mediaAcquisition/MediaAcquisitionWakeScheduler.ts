import { AppWindow } from '../core/window'

// The renderer starts its own runner after PageMain is ready. This scheduler is
// only a recovery mechanism for renderer reloads / hidden windows, so waking
// it every few seconds (or immediately during cold start) competes with the
// initial account and drive loading work.
const WAKE_INTERVAL_MS = 60_000
let timer: ReturnType<typeof setInterval> | undefined

/**
 * The renderer owns provider credentials and execution, while the main
 * process owns the durable wake-up cadence. This re-triggers a fresh renderer
 * after reloads or a hidden window without duplicating the DB lease.
 */
export function startMediaAcquisitionWakeScheduler(): void {
  if (timer) return
  const wake = () => {
    const window = AppWindow.mainWindow
    if (window && !window.isDestroyed()) window.webContents.send('mediaAcquisition:wake')
  }
  timer = setInterval(wake, WAKE_INTERVAL_MS)
}

export function stopMediaAcquisitionWakeScheduler(): void {
  if (timer) clearInterval(timer)
  timer = undefined
}
