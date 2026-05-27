import useSettingStore from '../setting/settingstore'
import useMusicLibraryStore from '../store/musiclibrary'
import { useMediaLibraryStore } from '../store/medialibrary'
import MusicScanner from './musicScanner'
import { MediaScanner } from './mediaScanner'
import DB from './db'
import DebugLog from './debuglog'

/**
 * 文件/文件夹被删除（移入回收站或彻底删除）时调用。
 * 负责：
 *   1) 停止可能正在跑的扫描任务（如果命中被删树，避免脏数据继续入库）
 *   2) 从 setting.uiMusicAutoScanFolders 移除被删的"手动扫描注册项"
 *   3) 从 mediaStore.folders 移除被删的"视频媒体库根文件夹"
 *   4) 从 musicStore.tracks / mediaStore.mediaItems 移除 file_id 命中的刮削项
 *
 * 注意：阿里云盘等只返回成功删除的 file_id，子树内的曲目 file_id 在云端虽然消失，
 * 但本地无法直接得到。我们用"file_id 完全相等"或"parent_file_id ∈ deletedSet"两条规则
 * 做一层清理。剩余的脏数据在用户播放时会报错，由 UI 触发"再清"。
 */
export async function handleFilesDeleted(
  user_id: string,
  drive_id: string,
  file_ids: string[]
): Promise<void> {
  if (!user_id || !drive_id || !file_ids || file_ids.length === 0) return
  const deletedSet = new Set(file_ids.filter(Boolean))
  if (deletedSet.size === 0) return

  try {
    await stopScansIfHit(user_id, drive_id, deletedSet)
  } catch (e) {
    DebugLog.mSaveWarning('handleFilesDeleted stopScansIfHit: ' + (e as Error).message)
  }
  try {
    await purgeMusicAutoScanRegistry(user_id, drive_id, deletedSet)
  } catch (e) {
    DebugLog.mSaveWarning('handleFilesDeleted purgeMusicAutoScanRegistry: ' + (e as Error).message)
  }
  try {
    await purgeVideoMediaLibrary(user_id, drive_id, deletedSet)
  } catch (e) {
    DebugLog.mSaveWarning('handleFilesDeleted purgeVideoMediaLibrary: ' + (e as Error).message)
  }
  try {
    await purgeMusicLibrary(user_id, drive_id, deletedSet)
  } catch (e) {
    DebugLog.mSaveWarning('handleFilesDeleted purgeMusicLibrary: ' + (e as Error).message)
  }
}

async function stopScansIfHit(_user_id: string, _drive_id: string, _deletedSet: Set<string>) {
  // 简化：只要有删除发生且扫描在跑，就停掉。
  // 因为继续扫描可能基于已失效的 parent_file_id，且阿里云盘 walk 是全盘 server-side，
  // 部分进度损失影响很小（下次 incremental 还会重新跑）。
  if (MusicScanner.getInstance().isScanning) MusicScanner.getInstance().stopScan()
  if (MediaScanner.getInstance().isCurrentlyScanning) MediaScanner.getInstance().stopScan()
}

async function purgeMusicAutoScanRegistry(
  user_id: string,
  drive_id: string,
  deletedSet: Set<string>
) {
  const setting = useSettingStore()
  const list = setting.uiMusicAutoScanFolders || []
  const next = list.filter((f) => {
    if (!f) return false
    if (f.user_id !== user_id) return true
    if (f.drive_id !== drive_id) return true
    if (deletedSet.has(f.file_id)) return false
    return true
  })
  if (next.length !== list.length) {
    await setting.updateStore({ uiMusicAutoScanFolders: next })
  }
}

async function purgeVideoMediaLibrary(
  user_id: string,
  drive_id: string,
  deletedSet: Set<string>
) {
  const mediaStore = useMediaLibraryStore()
  // 1) 删除作为"媒体库根"的文件夹（这些都是用户手动扫过的视频文件夹）
  const removedFolderIds = new Set<string>()
  for (const f of (mediaStore.folders || [])) {
    if (!f) continue
    if (f.userId !== user_id) continue
    if (f.driveId !== drive_id) continue
    if (deletedSet.has(f.fileId)) {
      removedFolderIds.add(f.id)
    }
  }
  // 2) 删除 mediaItems 中 file_id 命中、或所属 folder 已被移除的视频
  const items = mediaStore.mediaItems || []
  const itemIdsToRemove: string[] = []
  for (const it of items) {
    if (!it) continue
    const itemAny = it as any
    if (itemAny.userId && itemAny.userId !== user_id) continue
    if (itemAny.driveId && itemAny.driveId !== drive_id) continue
    const fileId = itemAny.fileId || itemAny.id
    const folderId = itemAny.folderId
    if (deletedSet.has(fileId) || (folderId && removedFolderIds.has(folderId))) {
      itemIdsToRemove.push(itemAny.id)
    }
  }
  if (itemIdsToRemove.length && typeof (mediaStore as any).removeMediaItem === 'function') {
    for (const id of itemIdsToRemove) (mediaStore as any).removeMediaItem(id)
  }
  if (removedFolderIds.size > 0 && typeof (mediaStore as any).removeFolder === 'function') {
    for (const id of removedFolderIds) (mediaStore as any).removeFolder(id)
  }
}

async function purgeMusicLibrary(
  user_id: string,
  drive_id: string,
  deletedSet: Set<string>
) {
  const musicStore = useMusicLibraryStore()
  const ids: string[] = []
  for (const t of musicStore.tracks) {
    if (!t) continue
    if (t.user_id !== user_id) continue
    if (t.drive_id !== drive_id) continue
    if (deletedSet.has(t.file_id) || (t.parent_file_id && deletedSet.has(t.parent_file_id))) {
      ids.push(t.id)
    }
  }
  if (ids.length === 0) return
  if (typeof (musicStore as any).removeTracksByIds === 'function') {
    ;(musicStore as any).removeTracksByIds(ids)
  }
  // 同步 Dexie
  try {
    await DB.deleteMusicTracksByIds(ids)
  } catch (e) {
    DebugLog.mSaveWarning('DB.deleteMusicTracksByIds: ' + (e as Error).message)
  }
}
