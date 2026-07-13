import useBookLibraryStore from '../store/booklibrary'
import useSettingStore from '../setting/settingstore'
import UserDAL from '../user/userdal'
import BookScanner from './bookScanner'
import DebugLog from './debuglog'

const FIRST_RUN_DELAY_MS = 10 * 1000
const RECHECK_INTERVAL_MS = 30 * 60 * 1000

let started = false
let timer: number | undefined

export function bootstrapBookLibrary() {
  if (started) return
  started = true
  const store = useBookLibraryStore()
  void store.loadFromDB().then(() => {
    setTimeout(() => triggerIfDue(), FIRST_RUN_DELAY_MS)
    timer = window.setInterval(() => triggerIfDue(), RECHECK_INTERVAL_MS)
  })
}

export function shutdownBookLibrary() {
  if (timer) {
    window.clearInterval(timer)
    timer = undefined
  }
  started = false
}

export async function triggerBookScanIfDue(force: boolean = false): Promise<boolean> {
  return triggerIfDue(force)
}

async function triggerIfDue(force: boolean = false): Promise<boolean> {
  const store = useBookLibraryStore()
  const setting = useSettingStore()
  if (store.isScanning || (!force && !setting.uiLibraryAutoScanBook)) return false

  const disabled = new Set(setting.uiLibraryAutoScanBookDisabledUsers || [])
  const users = await UserDAL.GetUserListFromDB()
  const allowed = new Set(users.filter((user) => user?.user_id && !disabled.has(user.user_id)).map((user) => user.user_id))
  if (!allowed.size) return false

  if (!force && setting.uiLibraryIncrementalScan) {
    const intervalMs = Math.max(1, setting.uiLibraryScanIntervalHours) * 60 * 60 * 1000
    if (store.lastScanAt && Date.now() - store.lastScanAt < intervalMs) return false
  }

  try {
    await BookScanner.getInstance().scanAllUsers({ userIdAllowList: allowed, silent: true })
    return true
  } catch (e) {
    DebugLog.mSaveWarning('bookLibrary auto scan failed: ' + (e as Error).message)
    return false
  }
}
