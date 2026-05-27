import { useMediaLibraryStore } from '../store/medialibrary'
import useSettingStore from '../setting/settingstore'
import { MediaScanner } from './mediaScanner'
import type { MediaLibraryFolder } from '../types/media'
import type { IAliGetFileModel } from '../aliapi/alimodels'
import DebugLog from './debuglog'

const FIRST_RUN_DELAY_MS = 12 * 1000
const RECHECK_INTERVAL_MS = 30 * 60 * 1000
const LS_LAST_SCAN = 'mediaLibrary.autoScan.lastAt'

let started = false
let timer: number | undefined

function readLastScanAt(): number {
  try {
    const v = localStorage.getItem(LS_LAST_SCAN)
    if (!v) return 0
    const n = Number(v)
    return Number.isFinite(n) ? n : 0
  } catch {
    return 0
  }
}

function writeLastScanAt(ts: number) {
  try { localStorage.setItem(LS_LAST_SCAN, String(ts)) } catch {}
}

function folderToAliModel(folder: MediaLibraryFolder): IAliGetFileModel {
  return {
    __v_skip: true,
    drive_id: folder.driveId,
    file_id: folder.fileId,
    parent_file_id: '',
    name: folder.name,
    namesearch: (folder.name || '').toLowerCase(),
    ext: '',
    mime_type: '',
    mime_extension: '',
    category: 'folder',
    icon: 'iconfile-folder',
    file_count: 0,
    size: 0,
    sizeStr: '',
    time: folder.scanDate ? new Date(folder.scanDate).getTime() : Date.now(),
    timeStr: '',
    starred: false,
    isDir: true,
    thumbnail: '',
    description: '',
    path: folder.path || '',
    user_id: folder.userId || ''
  } as IAliGetFileModel
}

export function bootstrapMediaLibrary() {
  if (started) return
  started = true
  setTimeout(() => triggerIfDue(), FIRST_RUN_DELAY_MS)
  timer = window.setInterval(() => triggerIfDue(), RECHECK_INTERVAL_MS)
}

export function shutdownMediaLibrary() {
  if (timer) {
    window.clearInterval(timer)
    timer = undefined
  }
  started = false
}

export async function triggerMediaScanIfDue(force: boolean = false): Promise<boolean> {
  return triggerIfDue(force)
}

async function triggerIfDue(force: boolean = false): Promise<boolean> {
  const setting = useSettingStore()
  const mediaStore = useMediaLibraryStore()
  if (mediaStore.isScanning) return false

  const followManual = !!setting.uiLibraryFollowManualScans
  const totalManualFolders = (mediaStore.folders || []).filter((f) => f && f.driveServerId !== 'local').length
  // 路径选择：
  //   1) force → 全部
  //   2) 总开关开 → 全部
  //   3) 总开关关 + followManual + 有已扫文件夹 → 全部
  //      （MediaScanner 的"已扫文件夹"== mediaStore.folders，本身就是用户手动扫过的）
  //   4) 其余：跳过
  if (!force && !setting.uiLibraryAutoScanVideo) {
    if (!(followManual && totalManualFolders > 0)) return false
  }

  const incremental = !force && setting.uiLibraryIncrementalScan
  if (incremental) {
    const intervalMs = Math.max(1, setting.uiLibraryScanIntervalHours) * 60 * 60 * 1000
    const last = readLastScanAt()
    if (last && Date.now() - last < intervalMs) return false
  }

  const disabledUsers = new Set(setting.uiLibraryAutoScanVideoDisabledUsers || [])
  const folders = (mediaStore.folders || []).filter((f) => {
    if (!f) return false
    if (f.driveServerId === 'local') return false
    if (f.userId && disabledUsers.has(f.userId)) return false
    return true
  })
  if (!folders.length) {
    writeLastScanAt(Date.now())
    return false
  }

  const scanner = MediaScanner.getInstance()
  let scanned = 0
  for (const f of folders) {
    try {
      await scanner.scanFolder(folderToAliModel(f), f.driveServerId, {
        incremental,
        silent: true
      })
      scanned += 1
    } catch (e) {
      DebugLog.mSaveWarning('mediaLibrary auto scan folder failed: ' + (e as Error).message)
    }
  }
  writeLastScanAt(Date.now())
  return scanned > 0
}
