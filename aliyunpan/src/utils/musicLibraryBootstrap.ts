import useMusicLibraryStore from '../store/musiclibrary'
import useSettingStore from '../setting/settingstore'
import MusicScanner from './musicScanner'
import UserDAL from '../user/userdal'
import DebugLog from './debuglog'

const FIRST_RUN_DELAY_MS = 8 * 1000
const RECHECK_INTERVAL_MS = 30 * 60 * 1000

let started = false
let timer: number | undefined

export function bootstrapMusicLibrary() {
  if (started) return
  started = true
  const store = useMusicLibraryStore()
  void store.loadFromDB().then(() => {
    setTimeout(() => triggerIfDue(), FIRST_RUN_DELAY_MS)
    timer = window.setInterval(() => triggerIfDue(), RECHECK_INTERVAL_MS)
  })
}

export function shutdownMusicLibrary() {
  if (timer) {
    window.clearInterval(timer)
    timer = undefined
  }
  started = false
}

export async function triggerMusicScanIfDue(force: boolean = false): Promise<boolean> {
  return triggerIfDue(force)
}

async function resolveAllowedUserIds(): Promise<Set<string>> {
  const setting = useSettingStore()
  const disabled = new Set(setting.uiLibraryAutoScanMusicDisabledUsers || [])
  const users = await UserDAL.GetUserListFromDB()
  const allowed = new Set<string>()
  for (const u of users) {
    if (!u || !u.user_id) continue
    if (!disabled.has(u.user_id)) allowed.add(u.user_id)
  }
  return allowed
}

async function triggerIfDue(force: boolean = false): Promise<boolean> {
  const store = useMusicLibraryStore()
  const setting = useSettingStore()
  if (store.isScanning) return false

  const allowed = await resolveAllowedUserIds()
  const manualFolders = (setting.uiMusicAutoScanFolders || []).filter((f) =>
    !!f && allowed.has(f.user_id)
  )
  const followManual = !!setting.uiLibraryFollowManualScans

  // 路径选择：
  //   1) force → 全盘
  //   2) 总开关开 → 全盘
  //   3) 总开关关 + followManual + 有手动文件夹 → 仅扫描这些
  //   4) 其余情况：跳过
  let mode: 'full' | 'manual' = 'full'
  if (!force && !setting.uiLibraryAutoScanMusic) {
    if (followManual && manualFolders.length > 0) mode = 'manual'
    else return false
  }

  if (mode === 'full' && allowed.size === 0) return false

  // 增量节流：只对 full 模式生效；manual 模式始终跑（开销小）
  let sinceMs = 0
  if (mode === 'full' && !force && setting.uiLibraryIncrementalScan) {
    const intervalMs = Math.max(1, setting.uiLibraryScanIntervalHours) * 60 * 60 * 1000
    if (store.lastScanAt && Date.now() - store.lastScanAt < intervalMs) return false
    sinceMs = store.lastScanAt || 0
  }

  try {
    if (mode === 'manual') {
      await MusicScanner.getInstance().scanRegisteredFolders({
        folders: manualFolders,
        userIdAllowList: allowed
      })
    } else {
      await MusicScanner.getInstance().scanAllUsers({
        force,
        sinceMs,
        userIdAllowList: allowed
      })
    }
    return true
  } catch (e) {
    DebugLog.mSaveWarning('musicLibrary auto scan failed: ' + (e as Error).message)
    return false
  }
}
