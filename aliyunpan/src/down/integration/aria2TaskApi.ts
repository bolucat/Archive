import { AriaRawCall, AriaConnect } from '../../utils/aria2c'
import { normalizeAriaTask, normalizeTaskFiles } from './taskTypes'
import type { DownloadTask, DownloadGlobalStat, DownloadTaskFile } from './taskTypes'

const TASK_FIELDS = [
  'gid', 'status', 'totalLength', 'completedLength', 'uploadLength',
  'downloadSpeed', 'uploadSpeed', 'numSeeders', 'seeder', 'connections',
  'numPieces', 'pieceLength', 'errorCode', 'errorMessage', 'dir',
  'files', 'bittorrent', 'verifiedLength', 'verifyIntegrityPending'
] as const

export const buildSelectFileOption = (indexes: number[]): { 'select-file': string } => ({
  'select-file': indexes
    .filter((index) => Number.isFinite(index) && index > 0)
    .join(',')
})

export const normalizeTaskListResult = (tasks: any[] = []): DownloadTask[] =>
  tasks.map((task) => normalizeAriaTask(task))

export async function getTaskStatus(gid: string): Promise<DownloadTask | null> {
  try {
    await AriaConnect()
    const task = await AriaRawCall('aria2.tellStatus', gid, [...TASK_FIELDS])
    return normalizeAriaTask(task)
  } catch {
    return null
  }
}

export async function getActiveList(): Promise<DownloadTask[]> {
  try {
    await AriaConnect()
    return normalizeTaskListResult(await AriaRawCall('aria2.tellActive', [...TASK_FIELDS]))
  } catch {
    return []
  }
}

export async function getWaitingList(offset = 0, num = 200): Promise<DownloadTask[]> {
  try {
    await AriaConnect()
    return normalizeTaskListResult(await AriaRawCall('aria2.tellWaiting', offset, num, [...TASK_FIELDS]))
  } catch {
    return []
  }
}

export async function getStoppedList(offset = 0, num = 200): Promise<DownloadTask[]> {
  try {
    await AriaConnect()
    return normalizeTaskListResult(await AriaRawCall('aria2.tellStopped', offset, num, [...TASK_FIELDS]))
  } catch {
    return []
  }
}

export async function getTaskFiles(gid: string): Promise<DownloadTaskFile[]> {
  try {
    await AriaConnect()
    return normalizeTaskFiles(await AriaRawCall('aria2.getFiles', gid))
  } catch {
    return []
  }
}

export async function changeTaskSelectedFiles(gid: string, indexes: number[]): Promise<void> {
  await changeTaskOption(gid, buildSelectFileOption(indexes))
}

export async function changeTaskOption(gid: string, options: Record<string, string>): Promise<void> {
  try {
    await AriaConnect()
    await AriaRawCall('aria2.changeOption', gid, options)
  } catch {}
}

export async function pauseTask(gid: string): Promise<void> {
  try {
    await AriaConnect()
    await AriaRawCall('aria2.forcePause', gid)
  } catch {}
}

export async function resumeTask(gid: string): Promise<void> {
  try {
    await AriaConnect()
    await AriaRawCall('aria2.unpause', gid)
  } catch {}
}

export async function removeTask(gid: string): Promise<void> {
  try {
    await AriaConnect()
    await AriaRawCall('aria2.forceRemove', gid)
    await AriaRawCall('aria2.removeDownloadResult', gid)
  } catch {}
}

export async function batchPauseTasks(gids: string[]): Promise<void> {
  await Promise.all(gids.filter(Boolean).map((gid) => pauseTask(gid)))
}

export async function batchResumeTasks(gids: string[]): Promise<void> {
  await Promise.all(gids.filter(Boolean).map((gid) => resumeTask(gid)))
}

export async function batchRemoveTasks(gids: string[]): Promise<void> {
  await Promise.all(gids.filter(Boolean).map((gid) => removeTask(gid)))
}

export async function getGlobalStat(): Promise<DownloadGlobalStat | null> {
  try {
    await AriaConnect()
    return (await AriaRawCall('aria2.getGlobalStat')) as DownloadGlobalStat
  } catch {
    return null
  }
}

export async function changeGlobalOption(options: Record<string, string>): Promise<void> {
  try {
    await AriaConnect()
    await AriaRawCall('aria2.changeGlobalOption', options)
  } catch {}
}
